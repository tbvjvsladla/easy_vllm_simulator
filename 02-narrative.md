# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 16건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/plan/plan_26102919_solar032_레시피_준비.md` | plan | 26102919 | 0 | 7203 | nv4 · safe · solar032 · solar032-nv4-safe-tp2 · tp2 | — |
| 2 | `docs/testlog/testlog_26103000_Solar032_TP2와_Full_판정.md` | testlog | 26103000 | 0 | 3270 | nv4 · safe · solar032 · solar032-nv4-safe-tp2 · tp2 | — |
| 3 | `docs/plan/plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화.md` | plan | 26102420 | 0 | 7459 | nv4 · tp2 | — |
| 4 | `docs/testlog/testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함.md` | testlog | 26102504 | 0 | 9039 | nv4 · tp2 | — |
| 5 | `docs/devlog/devlog_26103000_Solar032_정식이식과_실측_인계.md` | devlog | 26103000 | 0 | 2626 | solar032 · tp2 | — |
| 6 | `docs/plan/plan_26102919_멀티4종_v032_완주.md` | plan | 26102919 | 0 | 3259 | tp2 | — |
| 7 | `docs/plan/plan_26102921_Solar_i5_032_성능루브릭.md` | plan | 26102921 | 0 | 2336 | tp2 | — |
| 8 | `docs/benchmark/bench_report_26103000_solar-open2-250b-nota-nvfp4_GB10_0.32.0.md` | bench_report | 26103000 | 0 | 6138 | easy-vllm:0.32.0-cu133-aarch64-source-deps2 | — |
| 9 | `docs/devlog/devlog_26102504_PLE_mmap_재이식_full벤치_hint발행_서사.md` | devlog | 26102504 | 0 | 2920 | — | — |
| 10 | `docs/plan/plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context.md` | plan | 26102412 | 1 | 2576 | nv4 · tp2 | — |
| 11 | `docs/testlog/testlog_26102413_solar_open2_stock_0.31.0_아키텍처_미지원_판정.md` | testlog | 26102413 | 1 | 1921 | nv4 · tp2 | — |
| 12 | `docs/testlog/testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정.md` | testlog | 26102418 | 1 | 5865 | nv4 · tp2 | — |
| 13 | `docs/plan/plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증.md` | plan | 26102401 | 1 | 7359 | nv4 | — |
| 14 | `docs/testlog/testlog_26102418_4모델_싱글멀티_연속캠페인_결과요약_종결.md` | testlog | 26102418 | 1 | 3124 | nv4 | — |
| 15 | `docs/benchmark/bench_report_26102512_solar-open2-250b-nota-nvfp4_GB10_0.31.0.md` | bench_report | 26102512 | 1 | 5996 | — | — |
| 16 | `docs/plan/plan_26102700_코덱스_싱글8종_멀티4종_실측.md` (모호 해소) | plan | 26102700 | 1 | 7324 | nv4 · tp2 | docs/plan/plan_26102820_워커_예산과_무제한_A2A_리볼버.md |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

가장 이른 plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증은 네 모델을 싱글에서 먼저 판단한 뒤 멀티에서 지원 최대 context로 시험하려는 계획이었다. Solar는 싱글 기동 없이 산술 사전판정 후 TP2로 넘기기로 사람이 결정했다. 당시 du 기반 크기 가정은 뒤에 정정됐다(M3).

> [원문] plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증 §⚠ 사전에 짚을 전제 (착수 전 확증 대상)
> 2. **Solar 사전 판정**: NVFP4 286G > 노드 1대 메모리 → 싱글 불가를 산술 증거로 기록(기동 시도 ✗). 멀티 TP=2 에서도 가중치 143G/노드 > 121GiB 이면 CPU offload/PLE mmap 필요 여부를 판정하고, 불가 시 음성 정직 기록.

출발 하드웨어는 GB10 두 노드, 노드당 GPU 하나와 약121GiB 통합메모리, RoCE의 Ray TP2/EP다(plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context §목표; plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증 §NAS 사실). 현재 러너 MemTotal은124608MiB이고 호스트 드라이버580.178.04는 노드별 관측이다(01§1.4·1.5). OS는 미기록이다. 이미지 CUDA/torch는 호스트 좌표와 분리해00§0.4를 읽는다.

HF nota-ai/Solar-Open2-250B-Nota-NVFP4, checkout de88f6226788077e2d340204fd79d37720c9eda0, base upstage/Solar-Open2-250B다. 250B는 모델명이며 정확한 총/활성 파라미터 계수는 확보하지 않았다. 현재 NAS를 읽은 plan_26102919_solar032_레시피_준비 §모델별 근거는48층 중 GQA12/KDA36, native1048576과 native_weight_bytes153311254120을 기록한다. parser는 hybrid를 놓치므로 num_params를 정답으로 쓰지 않는다. safetensors total_size142.76GiB가 du286G를 정정했다(plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context §셀). compressed-tensors NVFP4와 제외530모듈의 기본BF16 혼합이며, PLE키 부재는00§0.4의 관측이다. MTP 구성은 확인되지 않았고 이번 spec은 꺼져 있다.

역사 stock0.31에는 SolarOpen2 등록이 없었고, 포크의 V1 logits·로더·배달 경로를 다시 이식해야 했다(W1~W7). 공식0.32에도 등록이 없어 새접점을 그라운딩했다. 모델카드의 전용 Upstage fork 00907fc9b982f973808a0193ad0f8c61fbb62854를 참조하되 다운그레이드 대신 런타임 자체이식을 고른 과거 결정을 잇는다. B300 TP4 카드수치는 GB10 TP2의 E가 아니다(plan_26102919_solar032_레시피_준비 §모델별 근거). 현재 사용자는 공식0.32 멀티4셀의 새 Full/hint를 승인했으며, 과거1M needle을 이번262k 셀의 새검증으로 세지 않는다(devlog_26103000_Solar032_정식이식과_실측_인계 ).

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-29T09:57:19Z(`camp-26102919-codex-multi4-v032` · evidence.event_campaign_boundary): campaigns/camp-26102919-codex-multi4-v032/campaign.yaml declared_utc 2026-10-29T09:57:19Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-29T15:01:25Z → 경계 = 앞선 값 2026-10-29T09:57:19Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-29T16:10:33Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) · sub = 2026-06-13T22:25:01Z → 2026-10-29T14:09:01Z(`sync_staging/sub_docs/logs/sub/events/2026-06.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-07.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-08.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-09.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-10.jsonl` · `sync_staging/sub_docs/logs/sub/events/thermal.jsonl` · `sync_staging/sub_docs/logs/sub/events/watchdog.jsonl` · 회수 미러가 가린 옛 로컬 사본 `docs/logs/sub/events/2026-09.jsonl`) — ⚠ sub(마지막 행 2026-10-29T14:09:01Z < 측정 끝 2026-10-29T15:34:50Z): 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-solar032-nv4-safe-tp2` | 2026-10-29T15:01:25Z | 4회 · 첫 2026-10-29T15:15:54Z | 없음 | `budget_clear` 2026-10-29T15:37:25Z | 10 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

### W1 역사0.31 stock 등록 부재

이것은 testlog_26102413_solar_open2_stock_0.31.0_아키텍처_미지원_판정의 기동 전 판정이며 새0.32 기동 실패가 아니다. 포크20파일 약2961줄에서 FusedMoEFactory·KDA·state2튜플·로더·파서 접점을0.31로 옮겼다. 해당 문서의 modeling파일 부재 주장은 뒤에 반증됐다(M4).

> [원문] testlog_26102413_solar_open2_stock_0.31.0_아키텍처_미지원_판정 §① 증상 (이미지 내부 실측)
> - 이미지 `/workspace/vllm-src/vllm/model_executor/models/registry.py` 의 solar 항목은 `SolarForCausalLM` **하나뿐** — `SolarOpen2*` 부재(원문 grep). 파서 `solar_open2`·로짓 프로세서도 부재 추정(포크 소속).

```hint-event
id: W1
kind: wall
증상: 기동 전 registry 조사에서 SolarOpen2 미지원 판정
서명: "의 solar 항목은 `SolarForCausalLM` **하나뿐** — `SolarOpen2*` 부재(원문 grep)"
원인: 0.31 stock registry에 모델 등록이 없음
해소: fork 델타를 out-of-tree 런타임 플러그인으로 자체이식
검증: testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §1. 아키텍처 미지원 판정과 이식 경로
전이등급: arch-invariant
출처: [testlog_26102413_solar_open2_stock_0.31.0_아키텍처_미지원_판정 §① 증상 (이미지 내부 실측), testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §1. 아키텍처 미지원 판정과 이식 경로]
```

### W2 역사 logits 인터페이스 거부

역사 초기에는 템플릿 강제 기능이 비어 있었고 뒤 S4에서 해결했다. 현재0.32는 V2 접점 재확정과 새 health/추론이 별도 근거다.

```hint-event
id: W2
kind: wall
증상: 첫0.31 기동에서 V1 processor를 거부
서명: "SolarOpen2TemplateLogitsProcessor is not a subclass of vllm.v1.worker.gpu.sample.logits_processor.LogitsProcessor"
원인: fork V1과0.31 V2 인터페이스 불일치
해소: 초기에는 processor 제외; plan_26102420 S4에서 V2로 재저작
검증: plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화 §정정·진행 기록 (2026-10-25 KST · S2~S4 완료)
전이등급: arch-invariant
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌), plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화]
```

### W3 역사 sub 플러그인 배달 누락

역사 표2의 두 배선 결함은 함께 고쳤다. 현재 sub도 트리플렛을 보내는 대신 허용된 런타임 패치쌍만 배달·무장한다. 다른 노드의 직접조사는 재현 절차가 아니다.

```hint-event
id: W3
kind: wall
증상: Ray worker가 모듈을 찾지 못함
서명: "No module named 'solar_open2_plugin'"
원인: 비추적 패치가 스냅샷에 안 들고 CONFIG_FILE=default가 모델 패치를 찾지 못함
해소: 패치쌍 스냅샷과 RUNTIME_PATCH_STEM/arm_patch/slave_forward 배선 교정
검증: 역사 testlog_26102418의 후속 기동/needle PASS
전이등급: judgment
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌), arm_patch.sh]
```

### W4 역사 conv1d loader 인자 불일치

같은 역사 표3의 로딩 벽이다. 현재0.32는 기존2인자 프로토콜과 KDA 접점을 새소스로 대조한 뒤 실제 양rank 적재를 별도로 관측했다.

```hint-event
id: W4
kind: wall
증상: 가중치 적재 중 loader 호출 실패
서명: "가중치 로딩 `weight_loader() missing 'loaded_shard_id'`"
원인: AutoWeightsLoader의2인자 호출과 conv1d의3인자 요구 충돌
해소: KDA subclass에서 tensor속성의 shard id를 읽는 어댑터
검증: 역사 testlog_26102418 §3 양노드71.6GiB 적재
전이등급: arch-invariant
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌)]
```

### W5 역사400k 단발 전력 가드

이는 과거0.31 1M 경로다. memwatch TRIP0은 전력가드 사살0을 뜻하지 않았다. 현재0.32에서는1M needle을 다시 수행하지 않았으며 이 역사성공을262k 전체길이의 새검증으로 읽지 않는다.

```hint-event
id: W5
kind: wall
증상: sub container kill137
서명: "400k 단발 프리필에서 slave 컨테이너 kill(137)"
원인: sub tp-watchdog gpu_pwr_sustained,81W/60버킷 정상작동
해소: 가드 유지,40k 이후32k/26k/20k/16k 증분 프리필과 재기동
검증: 역사 testlog_26102418 §4 최종1001129토큰 needle PASS
전이등급: arch-scaled
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌), testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §4. 장문 needle]
```

### W6 역사 reasoning 분리 공백

두 변경의 독립 기여는 분리 측정하지 않았다. 현재 content1자/reasoning516자 스모크는 전체 reasoning 품질·tool 조합의 인증이 아니다.

```hint-event
id: W6
kind: wall
증상: 초기 응답의 Thinking Process가 content에 섞임
서명: "가 섞이고 reasoning 필드는 비어 있음(파서 분리 미작동 — 후속)"
원인: preserved kwargs 미적용과 V2 processor 미이식이 함께 존재
해소: S4에서 V2와 preserved reasoning 경로를 함께 완결
검증: plan_26102420 S2~S4 완료 및 현재 Solar032 추론의 content/reasoning 분리
전이등급: arch-invariant
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌), plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화, testlog_26103000_Solar032_TP2와_Full_판정]
```

### W7 동기 스케줄링 및 미지원 기능 경계

역사 실패의 교훈을 새0.32 소스 접점으로 다시 확정한 조건이다. 이 셀에서 spec을 켜서 실패한 실험은 아니다. GDN mid-prefill checkpoint exporter도 미지원이므로 mamba-cache-mode:none과 prefix cache off로 경로를 배제했다. 이것은 지원확장이 아니다.

```hint-event
id: W7
kind: wall
증상: 역사 YAML false만으로 동기화되지 않았고 V2가 sync를 요구
서명: "- V2 logits FSM은스펙디코딩을거부하고동기스케줄링을요구하므로 speculative-config는없고CLI `--no-async-scheduling`을명시한다. YAML false값만으로이조건을보장하지않는다."
원인: V2 FSM은 sync를 요구하고 spec 미지원; YAML false만으로 보장 불가
해소: runner CLI --no-async-scheduling, spec 설정 없음
검증: 현재 Solar032 health200+추론 및 Full3
전이등급: arch-invariant
출처: [plan_26102919_solar032_레시피_준비 §공식0.32 접점 재확정, solar032-nv4-safe-tp2.sh]
```

### W8 역사 bench 메모리 예산 차단

옛14GiB KV/1M 형상의 예산 벽이며 arm_ceiling초과라는 설명은 반증됐다. 현재8GiB KV/262k는별도선언으로4096MiB Full을 통과했다. 숫자2048을 보편 처방으로 복사하지 않는다.

```hint-event
id: W8
kind: wall
증상: 과거 Solar Full에서4096MiB BLOCK
서명: "bench 예산 4096 MiB 가 BLOCK(Qwen TP1 · Solar multi 두 셀만"
원인: 실제 floor−bench≥abs_band 게이트, 과거Solar상한2128MiB
해소: 역사 Solar만2048MiB 선언으로 실행
검증: testlog_26102504 §2 표2 및 §1 Solar Full
전이등급: arch-scaled
출처: [testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2. 하네스 결함 · 처방]
```

### W9 새 CPU probe harness 구문오류

준비 도구 실패를 모델/GPU 실패로 세지 않는다. 수정 전 로그는 보존됐으며 유효 CPU검사를 반복해 성공을 고른 것이 아니다. CPU PASS의 native CUDA 미검증 한계는 후속 실서빙과 분리한다.

```hint-event
id: W9
kind: wall
증상: 유효 import 검사 전 첫 launch가 SyntaxError
서명: "CPU 노출 조건의 로그에는 `Failed to import from vllm._C: ModuleNotFoundError`도 있었다. 따라서 이 검사는 Python plugin 접점 호환만 확인하며 native CUDA core·커널 실행·실서빙 성공의 증거가 아니다. 메인 GPU probe는 별도 owner가 판정한다. 가중치 로드, GPU 모델, 추론, TP2, 서브 검사, Full은 여기서 실행하지 않았다. 최초 harness launch는 종료식 오타로 import 전에 SyntaxError가 났으며 수정 전 로그도 보존했다. 유효 import probe를 반복해 성공으로 고른 것이 아니다."
원인: 종료식 오타로 import 전에 중단
해소: 종료식 교정 후 단일 유효 CPU probe
검증: 7단계 import/registration PASS, GPU노출0
전이등급: judgment
출처: [plan_26102919_solar032_레시피_준비 §메인 stock 이미지 CPU import 검사]
```

### W10 새 provenance 정본 결속 차단

과거default0.26 hash는 배달 provenance 계약이다. 이를0.32 GPU/TP2 검증증거로 바꾸거나 global production을 승격하지 않았다.

```hint-event
id: W10
kind: wall
증상: 초기 실험해소값 stamp와 정식 sync 정본 불일치
서명: "초기helperstamp는실험해소값output/multi/resolved.json을사용했지만,정식sync계약은tracked assets/current-production-resolution.json의전역정본hash를소비한다. sync가기존계약대로비일치를차단해정식canonical정본과모델별yaml/sh/env로다시stamp했다. 전역정본의과거default0.26은실험엔진0.32의SHA증명이아니며임의승격하지않았다. 모델env의IMAGE_TAG/VLLM_REF0.32값이해시로결속되고,실제0.32sourcepin/호환성조사는compatibility.json과이미지프로브/서빙로그로따로증명한다. 이provenance통과를실제엔진버전·GPU·TP2성공으로해석하지않는다."
원인: runtime stamp와 sync가 참조한 resolution 권위가 다름
해소: 정식 tracked canonical baseline과 새모델 입력으로 restamp/verify
검증: 정식 stamp와 verify PASS; 실제0.32는 sourcepin/로그로 별도검증
전이등급: judgment
출처: [plan_26102919_solar032_레시피_준비 §정식 배달 provenance 결속의 범위]
```

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: 역사0.22 fork pin 채택 검토
기각사유: 0.31에서 다운그레이드라 자체이식을 선택; 실행 실패 아님
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정]
```

```hint-event
id: R2
kind: rejected
시도: Solar 코드를 모델 전용 이미지에 굽는 빌드패치안
기각사유: 순수Python serve시점 성립이라 런타임 패치 선택
출처: [plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화]
```

```hint-event
id: R3
kind: rejected
시도: 모델카드 triton backend 이식 검토
기각사유: 역사0.31 NVFP4 후보에 없어 marlin 사용; triton 실기동 실패로 세지 않음
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정]
```

```hint-event
id: R4
kind: rejected
시도: 싱글 노드 가중치 로드
기각사유: 142.76GiB 대 약121GiB로 산술배제, 실제기동0
출처: [plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context]
```

```hint-event
id: R5
kind: rejected
시도: 역사 기본 vLLM bench 첫 패스
기각사유: GuideLLM이 없어 Full무효; guidellm 명시 재측정
출처: [testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함]
```

```hint-event
id: R6
kind: rejected
시도: 400k 전력가드 우회
기각사유: 안전가드 유지, 증분으로 역사 장문시험
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정]
```

```hint-event
id: R7
kind: rejected
시도: 새0.32 spec 및 mid-prefill checkpoint 활성화
기각사유: V2/KDA 미지원 경로는 후보에서 제외; 실제 enabled실패 아님
출처: [plan_26102919_solar032_레시피_준비]
```

경로는 stock지원 부재를 자체이식으로 보완하고, 안전과 도구조건을 지킨 형상으로 좁혔다. 새0.32의262k·seq1은 성능최적화 완료가 아니라 보수 시작 형상의 종료된 실측이다.

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

```hint-event
id: M1
kind: misdiagnosis
증상: 역사 bench4096 BLOCK를 arm_ceiling 초과로 오진
서명: "arm_ceiling 9527/9296 초과"
원인: 예산의 arm ceiling과 bench 진입 floor 조건을 혼동
해소: 실제 floor−bench≥abs_band 게이트이며 당시 Solar bench상한2128MiB;2048MiB로측정
검증: testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2. 하네스 결함 · 처방
전이등급: judgment
현재지위: 반증
출처: [testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2. 하네스 결함 · 처방]
```

```hint-event
id: M2
kind: misdiagnosis
증상: 역사 실패한 sweep 뒤 stale PASS
서명: "가 실패한 sweep(rc=3) 뒤에도 과거 산출물로 PASS 를 냄(Solar 에서 실제 발생 — 폐기하고 재측정)"
원인: 판정기가 실패와 산출물 신선도를 구분하지 못함
해소: 해당 PASS폐기/재측정; 이후 sweep_incomplete와judge exit3/J12 교정
검증: testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §4. 후속 해소 · 정정
전이등급: judgment
현재지위: 반증
출처: [testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함]
```

```hint-event
id: M3
kind: misdiagnosis
증상: du286G를 실제 가중치로 받아들인 출발 가정
서명: "멀티 TP=2 에서도 가중치 143G/노드 > 121GiB 이면 CPU offload/PLE mmap 필요 여부를 판정"
원인: 파일시스템 du와 safetensors total_size를 혼동; du 과대 원인 자체는 미기록
해소: total_size142.76GiB, TP2약71.4GiB; 현재 엔진 모델71.6GiB/노드
검증: plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context §셀 및 testlog_26103000_Solar032_TP2와_Full_판정
전이등급: arch-invariant
현재지위: 반증
출처: [plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증, plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context, testlog_26103000_Solar032_TP2와_Full_판정]
```

```hint-event
id: M4
kind: misdiagnosis
증상: 역사 modeling파일 부재로 HF대조 불가라고 단정
서명: "`configuration_solar_open2.py` 만 있고 modeling 코드 없음(trust_remote_code 폴백 불가)"
원인: 체크포인트 파일목록 확인 누락
해소: modeling_solar_open2.py46KB 실재, HF수학 대조경로는 열려 있으나 KDA정밀동치 미수행
검증: testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §3. 정정 · hint 발행 결과
전이등급: judgment
현재지위: 반증
출처: [testlog_26102413_solar_open2_stock_0.31.0_아키텍처_미지원_판정, testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함, plan_26102919_solar032_레시피_준비]
```

> [원문] testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2. 하네스 결함 · 처방
> | 4 | `judge_bench.sh` 가 실패한 sweep(rc=3) 뒤에도 과거 산출물로 PASS 를 냄(Solar 에서 실제 발생 — 폐기하고 재측정) | **미해결 하네스 결함** — 판정기가 sweep rc/신선도를 검사해야 함. 드라이버에서 sweep rc≠0 이면 judge 건너뛰도록 보강 필요 |

교정 묶음 testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2의5항목을 검토했다. #1 기본도구 vllm→GuideLLM(과거무효Full 폐기), #2 예산4096→2048(과거형상만), #4 stalePASS폐기와후속신선도수리가 Solar에 해당한다. #3 Qwen E와 #5 single runner는 Solar수치의 교정이 아니다. §3 modeling파일 부재 정정(M4), §4 judge/classify 후속해소를 함께 반영했다. 초기 문서의 미해결 신선도 상태를 현재 상태로 승계하지 않는다. testlog_26102418_4모델_싱글멀티_연속캠페인_결과요약_종결 §하네스 교정4항목에서는 #3 runtime패치 sub배선, #4 멀티통로 절차가 관련되고 #1 싱글포트/#2YaRN은 Solar에 해당하지 않는다.

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

현재 TP2/EP/Marlin(V1~V4)은 모델별 근거로 다시 골랐고 이 셀에서 backend 스윕은 하지 않았다. GMU0.80(V5), context262144(V6), KV fp8(V7),8GiB/GPU(V8), eager(V9),seq1(V10),prefill2048(V11)은 손레버 보수 시작값이며 tuned가 아니다. 과거0.31 native1M/KV14GiB/seq1에서 새0.32의 작은 context/KV/프리필로 바뀌었다. 엔진이262k당2.58배 KV수용량677033토큰을 보고했지만8GiB가 수학적 최소라는 뜻은 아니다. 별도 토큰당바이트 필요량과 최소배수 계산은 계보에 없으므로 새로 만들지 않는다.

> [원문] plan_26102919_solar032_레시피_준비 §초기 안전 형상
> 새선언은context262144, batch1, prefill2048, KV8GiB/GPU, GMU0.80, eager, EP on, marlin, fp8_e4m3다. Solar native1M을강제하지않는다. 같은모델이과거14GiB/GPU에서약1.2M KV토큰을보고했으므로262k/8GiB는시작후확인할보수후보다. 이비교는새엔진KV측정을대신하지않는다. 고정KV는TP로다시나누지않는다. 과거weights+새KV+보수overhead24,800MiB로부하전정식예산게이트를통과해야하며필요조정은실제로그로한다.

현재 선언은 MemTotal124608MiB(러너실측)−weights73104−KV8192−overhead24800=floor18512MiB다(01§1.4 원장). weights/KV/overhead/floor는 예산입력이며 엔진 모델71.6GiB 및 KV8GiB reread와 구분한다. bench4096MiB는 별도선언이다. eager의 graph대조실험이나 capture피크 실측은 이 셀에 없으므로 graph불가·최속이라 주장하지 않는다. 조정 레버스윕은0이며 C1의warm반복3회는 설정스윕이 아니다.

mamba-cache-mode:none(V12)은 새로운checkpoint 미지원 경로 배제, logits/parser/preserved(V13~V17)는 이식의 기능 설정이다. CLI의 no-async/no-prefix를 YAML블록값과 혼동하지 않는다. spec off여서 acceptance length는N/A이며 손으로 넣은1.0이 아니다. 내부 roofline26.25/expected9.19는 현재 판정입력이다.

형상이 버틴 것은 TP2로 모델 적재가71.6GiB/노드이고 제한된KV8GiB와 보수overhead를 둔 채 안전가드를 통과했다는 관측이다(testlog_26103000_Solar032_TP2와_Full_판정). UMA에서 CPUoffload가 별도 VRAM을 제공한다고 가정하지 않는다. 과거1M/KV14GiB도 성공한 별도형상이므로 실패기전으로 적지 않는다. 과거400k 단발은 전력가드(W5), 과거bench4096은 예산floor(W8)로 막혔으며 현재의여러설정변경 중 어떤하나가 메모리를 얼마나 줄였는지는 분리하지 않았다.

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- du를 가중치 바이트로 쓰지 않는다 — M3 — safetensors total_size와 엔진 적재를 대조한다.
- stock SolarForCausalLM을 SolarOpen2 지원으로 읽지 않는다 — W1 — exactSHA registry를 확인한다.
- CPU import PASS를 GPU/TP2 성공으로 세지 않는다 — W9 — health·추론·양rank 적재 증거로 구분한다.
- sub에 모델 트리플렛을 복사해 패치 배달을 대신하지 않는다 — W3 — 정식 패치쌍/키체인을 따른다.
- YAML false만으로 sync/prefix 조건을 보장하지 않는다 — W7 — runner CLI와 실제 엔진 args를 확인한다.
- spec/checkpoint를 지원한 것으로 읽지 않는다 — R7 — 새접점 지원이 있어야 별도 범위를 연다.
- 전력가드나 예산바닥을 내리지 않는다 — W5/W8 — 형상과 프리필을 다시 도출한다.
- 실패한 sweep 뒤 남은 PASS를 재사용하지 않는다 — R5/M2 — 유효 GuideLLM·신선도·반복을 대조한다.
- 오래 걸리는 CIFS 가중치 로드를 즉시 hang으로 단정하지 않는다 — 현재 testlog의 약835초 READY — 안전갱신과 적재진행을 관측한다.
- 1M 역사 needle이나262k 설정을 이번1024/256 Full의 전체길이 품질검증으로 쓰지 않는다 — W5/03§3.4 — 관측범위를 표기한다.
- SCM dev버전을 다른 소스태그로 바꾸지 않는다 — 00§0.4 — exact40SHA를 재현좌표로 쓴다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: KDA gate lower bound와HF수학 정밀동치는 성립하는가
현재상태: 새 기능스모크/Full은 통과했으나 정밀대조는 미수행
다음관측: 같은작은입력으로HF와포트 중간상태/출력을대조
출처: [plan_26102919_solar032_레시피_준비, testlog_26103000_Solar032_TP2와_Full_판정]
```

```hint-event
id: Q2
kind: open-question
물음: 새0.32의262k 장문품질/tool 전체조합은 어떤가
현재상태: 설정/KV수용과짧은추론/1024·256 C1 Full만 관측; 과거1M은다른버전
다음관측: 별도승인된구체장문·tool회귀
출처: [plan_26102919_solar032_레시피_준비, testlog_26103000_Solar032_TP2와_Full_판정]
```

```hint-event
id: Q3
kind: open-question
물음: 상위vLLM이SolarOpen2/checkpoint/spec를정식지원하는가
현재상태: 현재exactSHA registry부재,checkpoint/spec배제
다음관측: 대상상위SHA의registry/KDA/V2접점조사
출처: [plan_26102919_solar032_레시피_준비, testlog_26103000_Solar032_TP2와_Full_판정]
```

```hint-event
id: Q4
kind: open-question
물음: 동시성과graph/KV최적점은 어디인가
현재상태: seq1/C1×3만측정, 설정레버스윕0
다음관측: 실제목표부하·메모리예산으로독립계획
출처: [plan_26102919_solar032_레시피_준비, testlog_26103000_Solar032_TP2와_Full_판정]
```

```hint-event
id: Q5
kind: open-question
물음: wholeimage및sub과거부모레시피바이트가동일한가
현재상태: SHA/torch/driver는관측동일,이미지ID/부모층/SCM날짜다름;sub원core미회수
다음관측: 필요한축만새localbuild와ABI스모크로검증
출처: [plan_26102919_solar032_레시피_준비, testlog_26103000_Solar032_TP2와_Full_판정]
```

현재 Full정의 충족과weak PASS는 열린판정이 아니다. 위 질문은 품질·지원·최적화·재현범위의 남은 한계다.
