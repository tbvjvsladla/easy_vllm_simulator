# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.32.0-g5df63a7203cd/gpt-oss-120b/gb10-1g1n-sub-native/qmxfp4-len8192-kvauto` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-28T22:57:30Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26102705-codex-single8 · gptoss120b-0191-c0 · sub · campaign |
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
| 자격 근거 | `output/single/benchlog/sweep_gpt120v0320_20261028T2143/level_01/post_health_gptoss120b-0191-c0.json` · `output/single/benchlog/sweep_gpt120v0320_20261028T2143/level_01/guidellm_gptoss120b-0191-c0.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_gpt120v0320_20261028T2143/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

GPT-OSS-120B를 단일 GB10에서 v0.32.0 소스 커밋 기반 MXFP4, 길이 8192, auto KV, spec off와 graph로 측정해 동시성 1 decode 35.86 tok/s를 얻었다. 원본 의존성 충돌은 별도 계층으로 수리했다(W1). gmu는 손으로 선언한 값이지 메모리 강제 cap이나 최적값이 아니다(V1).

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | gpt-oss-120b | sweep_index.meta(output/single/benchlog/sweep_gpt120v0320_20261028T2143) |
| HF repo | openai/gpt-oss-120b | 체크포인트 .git/config remote url(huggingface.co · /app/models/OpenAI/gpt-oss-120b → <manifest.nas_model_path>/OpenAI/gpt-oss-120b) |
| HF revision(체크포인트 git HEAD) | b5c939de8f754692c1647ca79fbf85e8c1e70f8a | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/OpenAI/gpt-oss-120b → <manifest.nas_model_path>/OpenAI/gpt-oss-120b) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-28T22:11:36Z 이전 마지막 이동 2025-09-17T02:41:16Z) |
| base_model | 미관측 | README 머리 YAML 에 base_model 없음(/app/models/OpenAI/gpt-oss-120b → <manifest.nas_model_path>/OpenAI/gpt-oss-120b) |
| base 슬러그 | gpt-oss-120b | model(gpt-oss-120b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | mxfp4 | sweep_index.meta(output/single/benchlog/sweep_gpt120v0320_20261028T2143) |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_gpt120v0320_20261028T2143) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_gpt120v0320_20261028T2143) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.32.0 | sweep_index.meta(output/single/benchlog/sweep_gpt120v0320_20261028T2143) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.32.0-g5df63a7203cd | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=VLLM_REF(40자 커밋 · docker history build-arg VLLM_REF) · exact_tag=measured image sha256:5534cf107221da5513496c6b4b47bbe1ef63a794c0034a0ac1a037ec17e59991: /workspace/vllm-src/.git HEAD and exact tag peeled to 5df63a7203cdb9328d824a27b682e77c7b9928be (create/cp, no start) · VLLM_REF=5df63a7203cd…(40자 커밋) → <직전 릴리스 0.32.0>-g<sha12> |
| 세그먼트 `model` | gpt-oss-120b | 서빙 yaml model(output/single/configs/gptoss120b-0191-c0.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/OpenAI/gpt-oss-120b → <manifest.nas_model_path>/OpenAI/gpt-oss-120b) · hf_repo(openai/gpt-oss-120b) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-sub-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qmxfp4-len8192-kvauto | derived(q·len·kv) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | sub | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | mxfp4 | /app/models/OpenAI/gpt-oss-120b → <manifest.nas_model_path>/OpenAI/gpt-oss-120b/config.json quantization_config quant_method=mxfp4 · vocab quant[mxfp4]←'mxfp4' |
| 축 `len` | 8192 | 서빙 yaml max-model-len(output/single/configs/gptoss120b-0191-c0.yaml) |
| 축 `kv` | auto | 서빙 yaml kv-cache-dtype 부재 관측(output/single/configs/gptoss120b-0191-c0.yaml) → 엔진 기본값 auto · vocab kv[auto]←'auto' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

GB10 통합 메모리에서 가중치와 KV가 같은 풀을 쓴다. 다른 메모리 구조에서는 여유를 다시 재야 한다. [plan_26102905_GPT120_서브_v0320_실행계획 §GPT-OSS-120B sub v0.32.0 실행계획]

MXFP4 checkpoint와 BF16 텐서가 혼합되어 커널 선택이 하드웨어와 빌드에 의존한다. 클램프 전후 trial은 통과했지만 다른 형상의 실패 기전은 대조하지 않아 미결이다. [devlog_26102907_GPT120_v0320_실측과_인계 §GPT-OSS-120B v0.32.0 sub 실측 인계]

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 원본 이미지 의존성 정합 확인 필요 | 별도 dependency repair 계층과 pip check | judgment | testlog_26102907_GPT120_v0320_Full_판정 §GPT-OSS-120B GB10 v0.32.0 sub 신규 Full 판정 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | commit | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | 5df63a7203cdb9328d824a27b682e77c7b9928be | docker history build-arg VLLM_REF |
| vLLM 빌드 계보 SHA(40자) | 5df63a7203cdb9328d824a27b682e77c7b9928be | VLLM_REF(40자 커밋 · docker history build-arg VLLM_REF) |
| 직전 릴리스(prev_release) | 0.32.0 | naming.vllm_build_input |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.32.1.dev0+g5df63a720.d20261028 | output/single/benchlog/sweep_gpt120v0320_20261028T2143/lite_engine_gptoss120b-0191-c0.log:49 · 기동 2026-10-28T21:43:46Z(컨테이너 시계 UTC 가정) · docs/logs/sub/sessions/gpt120-v0320-20261028T2143.json — 측정 창을 포함한 현재 serve 세션 안·첫 측정 이전 기동 배너 · 같은 값의 다른 로그 3건 · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| vLLM 설치 distribution 메타(출처별 관측 · 다운로드 wheel 증명 아님) | 0.32.1.dev0+g5df63a720.d20261028.cu133 | 빌드 원장(docker-image(sub-local):sha256:5534cf107221da5513496c6b4b47bbe1ef63a794c0034a0ac1a037ec17e59991:/opt/easy-vllm/build_ledger.json · 빌드 시 원장(후속 이미지 층은 별도 관측) · measured(build-time, in-image)) vllm.dist_version(빌드 시점 measured · 이미지 안 배포 메타) |
| 체크포인트 revision(HF · git HEAD) | b5c939de8f754692c1647ca79fbf85e8c1e70f8a | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/OpenAI/gpt-oss-120b → <manifest.nas_model_path>/OpenAI/gpt-oss-120b) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-28T22:11:36Z 이전 마지막 이동 2025-09-17T02:41:16Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.32.0-cu133-aarch64-source-deps1 | 셀 env(output/single/envs/.env.gptoss120b-0191-c0) IMAGE_TAG |
| 이미지 digest | sha256:5534cf107221da5513496c6b4b47bbe1ef63a794c0034a0ac1a037ec17e59991 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.07 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | sweep_index.meta.driver_version(output/single/benchlog/sweep_gpt120v0320_20261028T2143 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 드라이버(노드별 관측) | 이 측정에 결속된 노드별 입력 미관측 | 이 측정에 결속된 노드별 구조화 관측 입력 없음(다른 시점 관측의 부재를 뜻하지 않음) |
| ⚠ 드라이버 관측 ≠ 선언 | 대조 불가 — 선언 580.178.04 · 이 측정에 결속된 관측 입력 없음 | 선언: sweep_index.meta.driver_version(output/single/benchlog/sweep_gpt120v0320_20261028T2143 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| quantization_config 선언 대상(양자화 층 · 제외 목록 밖) | `mxfp4` | /app/models/OpenAI/gpt-oss-120b → <manifest.nas_model_path>/OpenAI/gpt-oss-120b/config.json quantization_config quant_method |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

빌드 입력의 전체 커밋은 소스 기준점이다. 실제 source probe는 변경 파일 17개와 삭제 228개를 기록했으므로 SHA만으로 컴파일된 바이트의 동일성을 보장하지 않는다. 빌드 입력은 위의 전체 커밋을 참조한다. 빌드 원장 distribution 메타는 editable 설치의 빌드 시점 관측이며 다운로드한 기준 wheel 관측이 아니다. 실행 중 module 및 distribution 문자열은 별도 관측이다. SCM 개발 표기를 공식 새 릴리스로 읽지 않는다. 선행 GPU probe의 driver 580.178.04는 측정 세션에 결속된 관측이 아니다. sweep의 CUDA 132와 실행 컨테이너 torch CUDA 13.3은 서로 다른 기록이며 차이의 원인은 미확정이다. 후속 dependency 계층은 빌드 원장 바깥이다. [testlog_26102907_GPT120_v0320_Full_판정 §GPT-OSS-120B GB10 v0.32.0 sub 신규 Full 판정]

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 4개 — `tuned` 1 · `inherited` 3.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.8 | inherited | 손레버 선언, 제거 실패 미측정 | V1 |
| `max-model-len` | 8192 | inherited | 손레버 후보, 최적화 미측정 | V2 |
| `kv-cache-memory-bytes` | 467380993 | tuned | 두 trial의 측정 기반 클램프 | V3 |
| `quantization` | mxfp4 | inherited | 체크포인트 고유 양자화 | V4 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

KV 바이트와 gpu-memory-utilization, max-model-len은 새 GPU에서 측정해 다시 도출한다. 필요조건으로 제거 실패가 확인된 노브는 없다. gmu와 길이는 손으로 고른 승계값이며 최적값이 아니다; 양자화는 checkpoint의 고유 형식이다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. manifest와 모델 경로 및 파일을 확인하고 RAM 게이트를 통과한다.
2. 이미지와 빌드 원장을 확인하고 다른 버전의 패치 필요성을 다시 검토한다.
3. 예산을 선언한 뒤 기동하여 health 200과 실제 응답을 확인한다.
4. KV와 부하를 다시 측정하고 실패 시 원시 로그를 보존한다.

빌드·버전 문제는 upstream-version-watch, 서빙·KV는 vllm-recipe-explorer, 성능은 adversarial-benchmark, 노드 사실은 terraforming_node로 보낸다.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.32.0-g5df63a7203cd/gpt-oss-120b/gb10-1g1n-sub-native/qmxfp4-len8192-kvauto` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26102705-codex-single8 · gptoss120b-0191-c0 · sub |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26102705_codex_single8__sub__gptoss120b_0191_c0 |
| work-manifest | docs/_evidence/camp_26102705_codex_single8__sub__gptoss120b_0191_c0.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-26T21:47:32Z |
| 승인 발화(전사) | 사용자: 싱글 메인·서브 각각 서로 다른 4개 모델 총 8개 고유 모델, 모든 모델 Full bench 성공 후 hint 태그 발행까지 수행 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/skills/adversarial-benchmark/SKILL.md:33`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `independent-native-sub-finalchecker`(저작자 `sub-native-draft-author` 아님) · 주장 61건 · 지적 9건 중 고침 9건 · 열림 0 · 2026-10-29T00:31:45Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
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

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.32.0-g5df63a7203cd/gpt-oss-120b/gb10-1g1n-sub-native/qmxfp4-len8192-kvauto` — 꼬리 0토큰.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

이 셀은 기본 이름 축 외에 근거가 충분한 구분 꼬리를 고르지 않았다. 빈 꼬리는 다른 노브의 최적성을 뜻하지 않는다.
