# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.30.0/ornith-1.5-35b-a3b/gb10-1g1n-sub-native/qbf16-len262144-kvauto-moetriton-kv8g` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-10T22:50:00Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26101104-hint-cd · ornith15-35b-v030 · sub · campaign |
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
| 자격 근거 | `output/single/benchlog/sweep_ornith15-35b-v030/level_01/post_health_ornith15-35b-v030.json` · `output/single/benchlog/sweep_ornith15-35b-v030/level_01/guidellm_ornith15-35b-v030.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_ornith15-35b-v030/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Ornith 35B-A3B 모델(bf16, 하이브리드 MoE)을 GB10 ×1 · TP=1 단일 노드에서 vLLM 소스빌드(빌드 입력 v0.30.0) 위에 컨텍스트 262144 · KV dtype auto · MoE 백엔드 triton 으로 서빙했고, full 벤치 동시성 1 decode 30.82 t/s 가 측정됐다(판정 PASS 는 weak 권한의 표시).
가장 비싼 벽은 기본 MoE 백엔드(FlashInfer CUTLASS)가 컴파일 직후 호스트 메모리를 급락시켜 호스트 워치독이 컨테이너를 사살한 것이고, MoE 백엔드를 triton 으로 고정해 넘었다(W2, V11) — 기전은 미확정(Q1).
가장 오해하기 쉬운 값은 limit-mm-per-prompt 위 yaml 주석의 '사살 회피' 설명으로, 멀티모달을 끄고도 죽어 기각된 옛 판단이며 이 값의 지위는 승계값이다(M1, V5).

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | ornith-1.5-35b-a3b | sweep_index.meta(output/single/benchlog/sweep_ornith15-35b-v030) |
| HF repo | ornith-ai/Ornith-1.5-35B-A3B | 체크포인트 .git/config remote url(huggingface.co · /app/models/ornith-ai/Ornith-1.5-35B-A3B → <manifest.nas_model_path>/ornith-ai/Ornith-1.5-35B-A3B) |
| HF revision(체크포인트 git HEAD) | fbb995a79eedd569a5edc5f2af9644c0fa1124fc | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/ornith-ai/Ornith-1.5-35B-A3B → <manifest.nas_model_path>/ornith-ai/Ornith-1.5-35B-A3B) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-10T05:29:12Z 이전 마지막 이동 2026-08-19T23:35:43Z) |
| base_model | 미관측 | README 머리 YAML 에 base_model 없음(/app/models/ornith-ai/Ornith-1.5-35B-A3B → <manifest.nas_model_path>/ornith-ai/Ornith-1.5-35B-A3B) |
| base 슬러그 | ornith-1.5-35b-a3b | model(ornith-1.5-35b-a3b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | bf16 | naming-axis(q) — 측정 identity quantization = none(관측)(sweep_index.meta(output/single/benchlog/sweep_ornith15-35b-v030).quantization = none · quantization_source = measured(engine log)) — 비양자화 관측이라 이름 q 축 토큰으로 옮겼다 · q 축 = /app/models/ornith-ai/Ornith-1.5-35B-A3B → <manifest.nas_model_path>/ornith-ai/Ornith-1.5-35B-A3B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_ornith15-35b-v030) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_ornith15-35b-v030) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.30.0 | sweep_index.meta(output/single/benchlog/sweep_ornith15-35b-v030) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.30.0 | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=미관측 · VLLM_REF=v0.30.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | ornith-1.5-35b-a3b | 서빙 yaml model(output/single/configs/ornith15-35b-v030.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/ornith-ai/Ornith-1.5-35B-A3B → <manifest.nas_model_path>/ornith-ai/Ornith-1.5-35B-A3B) · hf_repo(ornith-ai/Ornith-1.5-35B-A3B) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-sub-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qbf16-len262144-kvauto-moetriton-kv8g | derived(q·len·kv) + tail(agent · naming.tail[] 근거) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | sub | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/ornith-ai/Ornith-1.5-35B-A3B → <manifest.nas_model_path>/ornith-ai/Ornith-1.5-35B-A3B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 262144 | 서빙 yaml max-model-len(output/single/configs/ornith15-35b-v030.yaml) |
| 축 `kv` | auto | 서빙 yaml kv-cache-dtype 부재 관측(output/single/configs/ornith15-35b-v030.yaml) → 엔진 기본값 auto · vocab kv[auto]←'auto' |
| 꼬리 `moetriton` | MoE 백엔드를 triton 으로 고정(기본 auto 는 컴파일 직후 호스트 메모리 급락으로 사살 관측) | `output/single/configs/ornith15-35b-v030.yaml` · 키 `moe-backend` = `triton` |
| 꼬리 `kv8g` | KV 캐시를 절대 8 GiB 로 클램프(kv-cache-memory-bytes) | `output/single/configs/ornith15-35b-v030.yaml` · 키 `kv-cache-memory-bytes` = `8589934592` |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

**(가) 이 결과가 성립하는 조건**

1. **통합메모리 + 호스트 워치독.** 노드 총량은 124,608 MiB 로 기록됐고, 호스트 `easy-vllm-memwatch` 가 컨테이너를 `docker kill` 했다. W2 의 사살 주체는 엔진 OOM 이 아니라 이 호스트 워치독이며, 원장(01 §1.4 이벤트 표 8 · 19 · 29행, 라벨 다름 행)에 기록된 규칙은 둘이다: 3회 중 1회는 절대 임계(rule=absolute, 임계 10240 MiB)가 mem_avail 10022 MiB 에서, 나머지 2회는 속도 기반 예측 규칙(rule=eta)이 mem_avail 11483 MiB(하락 속도 2149 MiB/s)와 11085 MiB(2342 MiB/s)에서 발화했다 — 두 건은 절대 임계 **위**에서 죽었다. 그러므로 '10240 MiB 만 피하면 안전하다'고 읽으면 안 된다. 임계가 없거나 규칙이 다르거나 메모리 구조가 다른 HW 에서는 같은 구성이 완주할지 관측이 없다. 근거 원장 docs/logs/sub/events(watchdog_trip 3건), testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3 · §6.
2. **bf16 비양자화 + 하이브리드 MoE.** 가중치 적재 64.69 GiB, KV 절대 클램프 8.0 GiB, 총 예약 72.69 GiB(03 §3.1 `lite.*`)로 노드 총량의 상당 부분을 차지한다. 노드 메모리가 더 작거나 이미 다른 프로세스가 쓰고 있으면 예산이 빠듯하다. 이 모델은 양자화 층 구성이 섞이지 않으므로 커널 경로가 양자화 혼합으로 갈리는 문제는 이 셀에 없다. 근거 testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §4.
3. **텍스트 전용 서빙.** 멀티모달 입력 한도를 0 으로 둔 형상이다. 이미지 · 비디오 요청이 필요하면 이 지도는 그 경우를 시험하지 않았다. 근거 ornith15-35b-v030.yaml.

**(나) 이 형상이 버티는 기전 대 다른 형상이 실패한 이유**

- `moe-backend: triton` — 시도 4(KV 8 GiB · 시퀀스 4 · 멀티모달 끔 + triton)가 READY 에 도달했고, 서빙 중 MemAvailable 최저는 약 37.2 GiB 였다. 기전: 왜 triton 경로가 컴파일 직후 급락을 만들지 않는지는 관측으로 가르지 않았다 — 지위: 현상 확정 · 기전 미결 — testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3 · §6.
- MoE 백엔드 auto(FlashInfer CUTLASS) — 이 셀과 MoE 백엔드만 다르다(시도 3 대 4) — 컴파일 직후 프로파일/워밍업 구간에서 MemAvailable 급락(testlog 가 '시리즈' 하나로 적은 시계열은 로드 후 약 46 GiB 가 30 초 만에 36 → 19 → 트립 12 GiB 이고 어느 시도의 것인지는 밝혀 있지 않다; 원장의 트립 시점 mem_avail 은 시도마다 10022 / 11483 / 11085 MiB — 할당이 회수 가능한 page cache 인지 상주 메모리인지는 가르지 않았다) — 지위: 현상 확정 · 기전 미결(워크스페이스 대 autotune) — 같은 문서 §3.
- KV 16 GiB · 시퀀스 8 · 멀티모달 허용(시도 1 · 2) 및 KV 8 GiB · 시퀀스 4 · 멀티모달 끔(시도 3), 모두 auto — 같은 지점에서 사살 — 크기 축소가 원인 해소가 아님을 보인다 — 지위: 확정(시도 3 대 4 대조), 단 시도 3 이 testlog 표 기준 KV · 시퀀스 · 배치 토큰 · 멀티모달을 동시에 바꿨고(시도 1 · 2 의 배치 토큰 값은 표에 없어 미기록) 개별 기여는 분리 불가 — 같은 문서 §3.


## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 첫 single_serve_up.sh 호출이 서빙 전에 종료(exit 4)했다 | --ground 로 해소 기록을 먼저 남긴다(사서 도구 부재로 unresolved · 참조 0건) | arch-invariant | testlog_26100111 §3 시도 1 이후 기동 진행 |
| 2 | W2 | 컴파일 직후 컨테이너가 exit 137 로 사망(호스트 워치독 docker kill) | serving yaml 에 moe-backend: triton 지정 | arch-locked | testlog_26100111 §3 시도 4 READY(health 200) · 10-10 testlog §2 READY |
| 3 | W3 | 롤백 down 의 5/5 단계가 FAIL, 세션이 남음 | --session-id 로 세션을 지정해 종료(제안은 그라운딩 검사를 세션 시작 앞으로 이동 — 반영 여부 미확인) | arch-invariant | 10-10 testlog §5 단일 세션 6/6 정리(세션이 하나인 경우) |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 시도 1 · 2 가 컴파일 직후 사살됨(exit 137) | 반증 | testlog_26100111 §3 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.30.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | 미관측 | 미관측 |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.30.1.dev0+gced6857af.d20260930 | output/single/benchlog/sweep_ornith15-35b-v030/sweep_index.json:17 meta.vllm_build(sweep_bench 가 이 측정의 레벨 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡은 값 — 배너 전용 규칙은 아니다) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 미관측 | 미관측 — attestation vllm_dist_version · native pip freeze 모두 없다(sweep_index.meta.vllm_build='0.30.1.dev0+gced6857af.d20260930' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| 체크포인트 revision(HF · git HEAD) | fbb995a79eedd569a5edc5f2af9644c0fa1124fc | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/ornith-ai/Ornith-1.5-35B-A3B → <manifest.nas_model_path>/ornith-ai/Ornith-1.5-35B-A3B) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-10T05:29:12Z 이전 마지막 이동 2026-08-19T23:35:43Z) |
| 빌드 트랙 | source-build | artifacts 선택자(image-history:docker history build-arg 관측(VLLM_REF=v0.30.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님)) |
| Dockerfile(선택자) | Dockerfile.source-build | image-history:docker history build-arg 관측(VLLM_REF=v0.30.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님) |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.30.0-cu133-aarch64-source | 셀 env(output/single/envs/.env.ornith15-35b-v030) IMAGE_TAG |
| 이미지 digest | sha256:055ce79184da480044540e62d385307a522d9e087b6726c51f2099e8a3d10705 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.07 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.173.02 | sweep_index.meta.driver_version(output/single/benchlog/sweep_ornith15-35b-v030 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 드라이버(노드별 관측) | 미관측 | 미관측 |
| ⚠ 드라이버 관측 ≠ 선언 | 미관측 | 미관측 |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/ornith-ai/Ornith-1.5-35B-A3B → <manifest.nas_model_path>/ornith-ai/Ornith-1.5-35B-A3B/config.json dtype |

**이 모델에 필요한 패치**(01 §1.1 관련성 · ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 — 둘 다 관측일 때만 확정): required 0개 · inactive-inferred 0개 · unknown 4개(unknown = 판정 신호 부족 — 필요 여부를 이 표로 단정하지 않는다).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) **빌드 트랙: source-build.** 사실 블록의 선택자는 이미지 history 관측(`VLLM_REF=v0.30.0` · `VLLM_VERSION` 없음)이다. 이 트랙을 고른 **사유는 이 계보 문서 5건에 적혀 있지 않다**(미기록) — 서브는 호스트에 이미 있던 이미지를 검증해 썼다(testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §1). 실린 Dockerfile 머리가 '왜 source-build 인가'로 적은 일반 사유는 NGC 알파 torch 와 prebuilt wheel 의 C++ ABI 불일치지만, 그 검증은 vLLM 0.22.1 시점이고 이 이미지(NGC 26.07 · v0.30.0)에서 재검증된 기록은 없다. 다른 트랙으로 갈 조건도 계보에 없다.

(2) **버전 문자열이 여럿이다.** 빌드 입력 ref `v0.30.0`(docker history build-arg — 이것이 재현 좌표) · 엔진 자기보고 `0.30.1.dev0+gced6857af.d20260930`(sweep_bench 가 엔진 로그에서 정규식으로 잡은 값이며 빌드 입력이 아니다) · 인증서 `vllm_version` 은 이 셀에 인증서가 없어 미관측 · wheel 메타 미관측. 소스 트리 `git describe` 가 `v0.30.0` 이라고 testlog 가 관측했다. dev 표기의 원인(setuptools_scm 해석)은 추론이며 메인 확인이 남았다(Q4). 40자 SHA 는 미관측이고 testlog 는 HEAD 를 9자(`ced6857af`)로만 적었다 — 단축 SHA 를 재현 좌표로 쓰지 말고 `VLLM_REF=v0.30.0` 을 쓴다.

(3) **arch-locked 값.** `TORCH_CUDA_ARCH=12.1a`(build-arg) · CPU arm64 · 드라이버 580.173.02(manifest 선언 — 노드별 관측값은 미관측). 네 HW 에서 다시 도출하는 방법은 계보에 없고, 추론으로 GPU compute capability 를 `nvidia-smi` 로 확인해 build-arg 를 맞추는 것이다 — 실행 검증 ✗.

(4) **선언 대 관측.** 실린 `Dockerfile.source-build` · `requirements.txt` 는 작업트리 바이트이고 이미지 history 와 11/20 만 일치한다. 서빙 이미지의 실제는 이미지 history 의 build-arg 와 층이다. 실린 Dockerfile 머리 가드는 다른 NGC 베이스(26.05)와 vLLM 0.26.0 키를 가리킨다(01 §1.2).

(5) **FACT 가 미관측으로 남긴 좌표.** 드라이버 노드별 관측 · 호스트 OS: 계보에 없음(미기록). 노드 메모리 총량: 124,608 MiB(testlog_26100111 §6, 예산 선언 mem_total 과 같은 값). 인터커넥트: 해당 없음(single). 호스트 층(드라이버 580.173.02)과 이미지 층(CUDA 13.3.1.008 · torch 2.13.0a0+9186a08 · NGC 26.07)은 다른 층이다.

(6) **핵심 의존 핀.** 사실 블록 build-arg 에 `RAY_VERSION=2.48.0`(single 에서는 미사용 패키지로 남는다는 것이 실린 Dockerfile 주석의 설명 — 그 파일의 신뢰도는 위 (4)). 실린 `requirements.txt` 에 `flashinfer-python==0.6.14` 핀이 있으나 이 파일이 이미지 안 바이트와 다르므로 **이미지의 실제 FlashInfer 버전은 미관측**이다. DeepGEMM 은 빌드 패치 10 번 헤더가 nv_dev 브랜치(a6b593d)를 가리키지만 이 모델의 요구라는 근거가 없다(01 §1.2). Triton 은 W2 의 해소 수단(`moe-backend: triton`)이 쓰는 경로이며 버전 핀은 미관측이다.


## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 11개 — `inherited` 10 · `declared-requirement` 1.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.80 | inherited | 손으로 정한 값(lockset gmu_source=hand · 손레버). yaml 주석의 산식으로 정했고 값을 바꿔 가며 잰 기록이 없다. 실제 KV 는 kv-cache-memory-bytes 클램프가 정한다 | V1 |
| `max-model-len` | 262144 | inherited | 체크포인트 네이티브 컨텍스트(yaml 머리 주석 ctx 262144)를 그대로 썼다. 줄여 본 기록 없음 | V2 |
| `max-num-seqs` | 4 | inherited | 손레버(lockset batch_source=hand-lever). 시도 3 에서 8 → 4 로 줄였으나 다른 변인과 동시에 바꿨고 값별 측정은 없다 | V3 |
| `max-num-batched-tokens` | 4096 | inherited | 시도 3 에서 정했고 이후 조정된 적 없다. 값별 측정 없음 | V4 |
| `limit-mm-per-prompt` | {"image": 0, "video": 0} | inherited | 텍스트 전용 서빙 선택. yaml 주석은 사살 회피 목적이라 적었지만 시도 3 이 이 값을 켠 채로도 사살돼 그 근거는 기각됐다(M1). 켜면 깨지는지는 triton 위에서 시험되지 않았다 | V5 |
| `kv-cache-memory-bytes` | 8589934592 | inherited | 손으로 정한 절대 클램프(lockset kv_source=hand). 16 GiB → 8 GiB 로 줄였고(시도 3) 값별 측정은 없다. 이 값에서 엔진 로그가 408,122 토큰 · 262,144 토큰 요청 기준 동시성 1.56x 를 보고한다 | V6 |
| `tensor-parallel-size` | 1 | inherited | GPU 1장 노드라 선택지가 없다. 조정 대상이 아니다 | V7 |
| `enable-auto-tool-choice` | true | inherited | 도구 호출 파서와 짝으로 켠 값. 10-10 · 10-01 측정에서 도구 호출을 시험한 기록은 없다 | V8 |
| `tool-call-parser` | qwen3_xml | inherited | 선택 근거가 계보에 없다. 도구 호출을 시험한 기록이 없어 동작 미확인 | V9 |
| `reasoning-parser` | qwen3 | inherited | 스모크에서 reasoning 필드가 분리됨(17*23 질의)을 1회 관측했다. 이 값을 빼 본 기록은 없다 | V10 |
| `moe-backend` | triton | declared-requirement | auto(FlashInfer CUTLASS)에서는 3회 사살, 시도 3 에 이 한 변인만 더한 시도 4 가 통과(W2). 호스트 워치독(절대 임계 10240 MiB와 속도 예측 규칙 둘, 00 §0.2) 아래에서의 관측이며 워치독이 없는 호스트에서의 거동은 관측 없음 | V11 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① **네 환경에서 다시 도출할 값(arch-scaled).** `kv-cache-memory-bytes`(V6), `gpu-memory-utilization`(V1), `max-model-len`(V2), `max-num-seqs`(V3), `max-num-batched-tokens`(V4). 도출 절차는 엔진 로그의 `GPU KV cache size`(토큰 수)와 `Maximum concurrency for <len> tokens` 줄을 읽어 목표 컨텍스트와 동시성에 맞춰 역산하는 것이다 — 이 셀은 이 값들을 스윕하지 않았으므로 도출 절차는 추론이다.
② **필요조건으로 확인된 값(declared-requirement).** `moe-backend: triton`(V11) — 빼면(auto) 이 호스트에서 3회 사살이 관측됐다(W2). 단 호스트 워치독(절대 임계와 속도 예측 규칙 둘, 00 §0.2) 아래에서의 관측이다. '명시하면 오히려 깨지는' 노브는 관측되지 않았다.
③ **승계값 · 손레버(inherited).** V1~V10 전부(V11 제외). 이 셀에서 값을 바꿔 가며 잰 기록이 없으며 최적이라는 뜻이 아니다. 특히 `limit-mm-per-prompt`(V5)는 yaml 주석이 사살 대책이라 적었지만 그 판단은 기각됐다(M1). 도구 호출 파서 두 노브(V8 · V9)는 시험된 적이 없고, `reasoning-parser`(V10)는 스모크 1회에서 reasoning 분리만 관측됐다.


## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **호스트 보호 장치와 메모리 예산을 먼저 확인한다.** 확인: 호스트의 워치독이 있는지와 그 규칙(이 호스트는 절대 임계 MemAvailable 10240 MiB 와 속도 기반 예측 규칙 둘이었고, 예측 규칙은 11 GiB 대에서 발화했다 — 임계 값만 보지 않는다), 예산 선언 절차가 있는지, 기동 중 `free` 로 MemAvailable 시계열을 본다. 분기: 임계가 없거나 크게 다르면 W2 의 사살은 재현되지 않을 수 있다 → 그대로 시험 / 임계가 같으면 MoE 백엔드를 triton 으로 고정해 시작한다 / 메모리 구조가 다르면 KV 클램프를 새로 도출한다(00 §0.5 ①). 선언 입력: 총량 · 가중치 · KV · overhead(이 셀은 124608 · 68572 · 8192 · 16384 MiB 로 floor 31460 MiB — 선언값, 01 §1.4).
2. **vLLM 버전이 다르면 W2 부터 다시 본다.** 확인: 같은 yaml 에서 `moe-backend` 만 auto 로 바꿔 한 번 기동(호스트 보호를 선언한 상태에서)하고 컴파일 직후 MemAvailable 을 비교한다 — 이 지도는 0.30.0 한 버전만 관측했다(Q6). 분기: 급락이 없으면 triton 고정은 불필요 / 있으면 triton 유지.
3. **이미지 · 빌드 충실도를 확인한다.** 확인: `docker history` 의 build-arg 와 01 §1.4 명령을 기준으로 새로 짓고, 실린 Dockerfile · requirements 를 그대로 쓰지 않는다(Q5). 빌드 패치 4건(DeepSeek-V4 계열 대상으로 선언)은 이 모델의 요구라는 근거가 없고 발화 서명도 관측되지 않았다 — 분기: 패치 없는 이미지로도 기동되면 빼고, 필요하면 업스트림 정식 지원 여부(트립와이어)부터 확인한다. 이 확인은 이 셀에서 해 본 적이 없다.
4. **기동 성공을 이렇게 판정한다.** `GET /health` 200 + chat completion 1회(`finish_reason=stop`, reasoning 필드 분리) + 엔진 로그에 가중치 적재 · KV 토큰 · 최대 동시성 줄(01 §1.4 (c)). 패치가 발화했음을 보여주는 로그 서명은 이 셀에서 관측되지 않았다.
5. **성능을 다시 잰다.** 같은 bench_mode(full) · 같은 도구에서만 비교한다(03 §3.5). 판정 PASS 는 weak 권한의 하한 위라는 표시다(Q7).

실패 신호별 행선지: 빌드 · 버전 핀 문제 → `upstream-version-watch` · 서빙 설정 · KV · 노브 문제 → `vllm-recipe-explorer` · 성능 기대 이하 → `adversarial-benchmark` · 노드 · 토폴로지 · 인터커넥트 문제 → `terraforming_node`.


## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.30.0/ornith-1.5-35b-a3b/gb10-1g1n-sub-native/qbf16-len262144-kvauto-moetriton-kv8g` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26101104-hint-cd · ornith15-35b-v030 · sub |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26101104_hint_cd__sub__ornith15_35b_v030 |
| work-manifest | docs/_evidence/camp_26101104_hint_cd__sub__ornith15_35b_v030.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-10T19:53:02Z |
| 승인 발화(전사) | 사용자(plan 승인 팝업 응답 전사) — "승인" (plan_26101104 · C·D 는 single 서브 자체 발행 · 근거 발화 10-10 "이번 6개 모델에 대한 E2E결과는 각각 힌트 태그 발행해줘") |
| 현행 full 정의 충족(측정 등급) | 미관측 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실 검증 Agent(서브 · 셀 C · plan_26101104)`(저작자 `hint 저작 Agent(서브 · 셀 C · plan_26101104)` 아님) · 주장 62건 · 지적 11건 중 고침 11건 · 열림 0 · 2026-10-10T22:53:54Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `HINT_MISSING_APPLIED_SET` | 빌드 패치 적용 집합 미관측 — 어느 빌드 패치가 이 이미지에 실제로 적용됐는지(빌드 원장 · 재구성) 읽지 못했다. 실린 패치 파일은 '적용됨' 이 아니라 '후보' 다(artifacts 가 기재). |
| `HINT_MISSING_BUILD_LEDGER` | 빌드 원장(`/opt/easy-vllm/build_ledger.json`) 부재 — 원장 도입 전 이미지다. 패치 적용 결과는 `applied_set.status: unobservable` 과 라벨된 재구성으로만 말한다(X9). |
| `HINT_MISSING_RESOLVE` | `output/<t>/resolved.json` 을 이 빌드에 묶지 못했다(부재 또는 다른 vLLM 버전의 해소값) — torch·NGC·CUDA 는 이미지 관측값만 싣는다. |
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

발행 경위(사실): 이 지도는 새 측정이 아니라 10-10 에 이미 끝난 측정과 10-01 서빙 기록을 정리해 발행한 것이다. 발행 시점에 재서빙 · 재측정은 하지 않았다. 저작은 이 셀을 측정한 노드의 에이전트가 맡았고, 사실 검증은 저작자가 아닌 별도 Agent 가 맡는다(00 §0.7 메타의 '독립 사실 검증' 칸이 그 결과를 싣는다). 이 지도에는 도서관 인용이 0건이다(그라운딩이 unresolved 로 기록됨, 02 §2.2 W1). 의견: 이 지도에서 가장 쓸모 있는 부분은 W2(MoE 백엔드)와 그 기전이 미결이라는 사실이고, 가장 약한 부분은 재현 충실도(Q5)다.

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.30.0/ornith-1.5-35b-a3b/gb10-1g1n-sub-native/qbf16-len262144-kvauto-moetriton-kv8g` — 꼬리 2토큰.

| 토큰 | 뜻 | 근거(서빙 설정) |
|---|---|---|
| `moetriton` | MoE 백엔드를 triton 으로 고정(기본 auto 는 컴파일 직후 호스트 메모리 급락으로 사살 관측) | `output/single/configs/ornith15-35b-v030.yaml` · 키 `moe-backend` = `triton` |
| `kv8g` | KV 캐시를 절대 8 GiB 로 클램프(kv-cache-memory-bytes) | `output/single/configs/ornith15-35b-v030.yaml` · 키 `kv-cache-memory-bytes` = `8589934592` |
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

결정론부가 같은 다른 셀과 이 셀을 가르는 노브로 두 가지를 골랐다: MoE 백엔드를 triton 으로 고정한 것(W2 의 해소 수단이자 이 셀의 유일한 declared-requirement)과 KV 절대 클램프를 8 GiB 로 둔 것(통합메모리에서 같은 q · len · kv 라도 KV 가 달라지면 예산이 달라지는 값). 후보 ple 은 이 모델에 PLE 키가 없어 가르지 않고, spec 은 꺼짐(speculative-config 부재)이라 가르지 않으며, graph 는 기본 cudagraph 이므로 가르는 노브가 아니다.
