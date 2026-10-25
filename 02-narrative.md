# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 10건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/testlog/testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함.md` | testlog | 26102504 | 0 | 3189 | 0.31.0 · 1m · easy-vllm:0.31.0-cu133-aarch64-source · nv4 · solar31 · solar31-nv4-1m-tp2 · tp2 | — |
| 2 | `docs/plan/plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화.md` | plan | 26102420 | 0 | 6650 | 0.31.0 · 1m · nv4 · solar31 · solar31-nv4-1m-tp2 · tp2 | — |
| 3 | `docs/plan/plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context.md` | plan | 26102412 | 0 | 2576 | 0.31.0 · 1m · nv4 · tp2 | — |
| 4 | `docs/devlog/devlog_26102504_PLE_mmap_재이식_full벤치_hint발행_서사.md` | devlog | 26102504 | 0 | 1524 | 0.31.0 · easy-vllm:0.31.0-cu133-aarch64-source | — |
| 5 | `docs/benchmark/bench_report_26102512_solar-open2-250b-nota-nvfp4_GB10_0.31.0.md` | bench_report | 26102512 | 0 | 5996 | 0.31.0 · easy-vllm:0.31.0-cu133-aarch64-source | — |
| 6 | `docs/testlog/testlog_26102418_4모델_싱글멀티_연속캠페인_결과요약_종결.md` | testlog | 26102418 | 1 | 3124 | 0.31.0 · 1m · nv4 | — |
| 7 | `docs/plan/plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증.md` | plan | 26102401 | 1 | 7359 | 1m · nv4 | — |
| 8 | `docs/testlog/testlog_26102413_solar_open2_stock_0.31.0_아키텍처_미지원_판정.md` | testlog | 26102413 | 3 | 1921 | 0.31.0 · 1m · easy-vllm:0.31.0-cu133-aarch64-source · nv4 · tp2 | — |
| 9 | `docs/testlog/testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정.md` (모호 해소) | testlog | 26102418 | 2 | 5865 | 0.31.0 · 1m · easy-vllm:0.31.0-cu133-aarch64-source · nv4 · solar31 · solar31-nv4-1m-tp2 · tp2 | — |
| 10 | `docs/testlog/testlog_26102412_4model_싱글캠페인_릴레이요약_종결.md` (모호 해소) | testlog | 26102412 | 2 | 1892 | 0.31.0 · 1m · nv4 | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

**목표** — 계보의 가장 이른 plan(plan_26102401)은 4모델을 싱글 → 멀티 순으로 "지원 최대 context" 로 띄우는 캠페인이었고, Solar-Open2-250B-Nota-NVFP4 는 싱글에서 산술로 사전판정한 뒤 곧바로 멀티 TP=2 로 넘기기로 사람이 정했다. 그 plan 이 Solar 에 대해 세운 가정은 아래다(뒤에 크기 가정이 틀린 것으로 정정됐다 — M3).

> [원문] plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증 §⚠ 사전에 짚을 전제 (착수 전 확증 대상)
> 2. **Solar 사전 판정**: NVFP4 286G > 노드 1대 메모리 → 싱글 불가를 산술 증거로 기록(기동 시도 ✗). 멀티 TP=2 에서도 가중치 143G/노드 > 121GiB 이면 CPU offload/PLE mmap 필요 여부를 판정하고, 불가 시 음성 정직 기록.

멀티 캠페인 plan(plan_26102412)이 목표를 Solar 포함 3셀의 TP=2 최대 context 기동으로 좁혔다.

> [원문] plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context §셀 (TP=2 이므로 노드 병렬 아님 · 순차)
> | 1 | solar-open2-nv4-1m-tp2 | Solar-Open2-250B-Nota-NVFP4 | 142.76 GiB → 노드당 ≈71.4 GiB | native 1,048,576 · YaRN 없음(rope_scaling 없음 · 추측 factor ✗) |

**HW 출발 조건** — NVIDIA GB10 × 2 노드 · 노드당 GPU 1 · 노드당 통합메모리 ~121GiB(plan_26102401 NAS 사실 절 — 통합메모리라 GPU 와 CPU 가 같은 메모리를 쓴다) · 인터커넥트 RoCE(plan_26102412 목표 · 측정 엔진 로그의 NCCL `NET/IB` 장치가 `RoCE` 로 찍히고 `GPU Direct RDMA Disabled`) · 분산 = Ray TP=2 + expert-parallel. 호스트 드라이버 580.178.04(양 노드 · attestation 관측)이고, 호스트 OS 는 계보에 **미기록**이다. 이미지 좌표(CUDA 13.3.1 · torch 2.13.0a0 · NGC 26.07)는 00 §0.4 의 별도 층이다. 원장 예산 선언의 mem_total_mib 124608 은 선언값이다(02 §2.5).

**모델 구조** — HF `nota-ai/Solar-Open2-250B-Nota-NVFP4` · revision `de88f6226788077e2d340204fd79d37720c9eda0`(00 §0.2) · base `upstage/Solar-Open2-250B`. 체크포인트 config(저작 시 직접 읽은 관측 — 계보 문서에는 층 구성이 적혀 있지 않다)는 48층 · `gqa_layers` 12개(0, 4, …, 44 — full attention) · 나머지는 KDA(Kimi Delta Attention 계열 선형 attention · `linear_attn_config`) · MoE routed expert 320 · 토큰당 8 · shared 1 · `use_rope: false` · `rope_scaling: null` · `max_position_embeddings: 1048576` 이고, MTP 층을 가리키는 키는 보이지 않는다. 총 파라미터는 이름의 250B 뿐이고 활성 파라미터는 **미기록**이다. 크기는 safetensors 실측 total_size 142.76 GiB(plan_26102412 · 측정 엔진 로그 `Checkpoint size: 142.78 GiB`)이며, NAS `du` 는 286G 로 두 배 가까이 부풀려 보고했다(M3). 양자화는 compressed-tensors `nvfp4-pack-quantized` · 제외 모듈 530개는 bfloat16 이다(00 §0.4). 엔진은 KDA 상태 때문에 이 모델을 mamba 계열 캐시로 다룬다(`Mamba cache mode is set to 'align' for SolarOpen2ForCausalLM` · lite_engine_head L72).

**stock 상태의 장벽**(자세히는 2.2)
- stock 0.31.0 레지스트리에 SolarOpen2 모델 · 파서가 없다(W1).
- 포크의 logits processor 는 v1 인터페이스라 0.31 V2 모델러너에서 거부된다(W2).
- 0.31 의 로더 · KDA API 가 포크(0.22.0) 이후 바뀌어 다시 써야 했다(파손 6종 · W4 가 그중 기동에서 드러난 것).
- 싱글 노드는 체크포인트가 메모리에 들어가지 않는다(R4 · 산술).

**선례와 사람의 결정** — 외부 선례는 HF 모델 카드 하나다: v0.22.0 기반 Upstage 포크(`UpstageAI/vllm@v0.22.0-solar-open2` · 도커 `upstage/vllm-solar-open2`)와 서빙 인자(`--moe-backend triton --enable-expert-parallel` · reasoning/tool 파서 `solar_open2` · logits processor)를 적고 1M context 에 YaRN 언급이 없다(testlog_26102413 §② 외부 확증). 카드의 HW 수 · 수치 · 멀티노드 지원 여부는 계보에 **미기록**이다. 사람은 stock 불가 판정 뒤 "2번(자체 이식) 수행" 을 골랐다(testlog_26102418 유효맥락) — 포크 핀은 0.31 → 0.22 다운그레이드라 불가였고(R1), 자체 이식 선판정 3신호가 go 였다. 이식물의 슬롯은 +1 런타임 패치(owner `vllm-recipe-explorer` · 이미지 불변)로 정해졌다(plan_26102420 §슬롯 판정).

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계 미관측** — 재생 모드(캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음)) — 캠페인 선언을 관측하지 못한다 · 경계 미관측 — 라벨(셀 이름)로 묶은 행 전부를 이 셀의 행으로 센다(같은 셀 이름의 앞선 캠페인 행이 섞였을 수 있다)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-25T03:13:55Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) · sub = 2026-06-13T22:25:01Z → 2026-10-24T00:02:35Z(`sync_staging/sub_docs/logs/sub/events/2026-06.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-07.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-08.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-09.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-10.jsonl` · `sync_staging/sub_docs/logs/sub/events/thermal.jsonl` · `sync_staging/sub_docs/logs/sub/events/watchdog.jsonl` · 회수 미러가 가린 옛 로컬 사본 `docs/logs/sub/events/2026-09.jsonl`) — ⚠ sub(마지막 행 2026-10-24T00:02:35Z < 측정 끝 2026-10-25T03:13:30Z): 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T05:57:40Z | 없음 | 없음 | `budget_clear` 2026-10-24T05:59:11Z | 2 |
| 2 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T06:00:00Z | 없음 | `budget_rejected` 1 | `budget_clear` 2026-10-24T06:04:36Z | 2 |
| 3 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T06:05:00Z | 없음 | 없음 | `budget_clear` 2026-10-24T06:10:17Z | 1 |
| 4 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T06:10:18Z | 1회 · 첫 2026-10-24T06:19:39Z | 없음 | `budget_clear` 2026-10-24T06:21:33Z | 4 |
| 5 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T06:22:22Z | 1회 · 첫 2026-10-24T06:37:16Z | 없음 | `budget_clear` 2026-10-24T06:49:21Z | 3 |
| 6 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T06:50:10Z | 4회 · 첫 2026-10-24T07:05:33Z | 없음 | `budget_clear` 2026-10-24T08:48:13Z | 3 |
| 7 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T14:34:00Z | 1회 · 첫 2026-10-24T14:46:16Z | 없음 | `budget_clear` 2026-10-24T14:47:54Z | 3 |
| 8 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T14:48:44Z | 1회 · 첫 2026-10-24T15:01:57Z | 없음 | `budget_clear` 2026-10-24T15:18:23Z | 3 |
| 9 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-24T20:51:16Z | 1회 · 첫 2026-10-24T21:04:14Z | 없음 | `budget_clear` 2026-10-24T21:20:21Z | 3 |
| 10 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-25T02:28:01Z | 1회 · 첫 2026-10-25T02:40:52Z | 없음 | `budget_clear` 2026-10-25T02:42:44Z | 3 |
| 11 | main | `smoke-solar31-nv4-1m-tp2` | 2026-10-25T02:43:38Z | 4회 · 첫 2026-10-25T02:57:42Z | 없음 | `budget_clear` 2026-10-25T03:13:54Z | 10 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

**기동 시도와 벽의 대응** — 원장은 이 셀 라벨로 기동 시도 11번을 묶는다(사살 · 트립 칸은 모두 `없음`이고 거부는 시도 2 의 `budget_rejected` 1건이다 — 워치독이 2026-10-24T06:04:36Z 에 `floor_mib/expires_epoch 파싱 실패` 사유로 남긴 행이며 같은 초에 그 시도의 `budget_clear` 가 찍혔다(watchdog.jsonl:759 · 사살 아님). 이 칸들은 메인 원장 기준이다. 서브 원장은 회수 미러 기준 2026-10-24T00:02:35Z 뒤 행이 없어(이 셀 기동 · 측정 창은 모두 그 뒤) 서브 쪽 사건은 이 표에 나올 수 없다). 엔진 로그로 시도와 벽을 맞출 수 있는 것은 둘이다: 시도 3(2026-10-24T06:05Z 창)은 `serve_fail_*/master_exited.log` 의 컨테이너 시각 06:05~06:06 과 겹치고 W3 서명을 담는다 · 시도 7(14:34Z 창)은 `master_failure.log` 의 14:34~14:46 과 겹치고 W7 서명을 담는다. 시도 5(06:22~06:49Z)의 닫힘 06:49:21Z 는 W5 의 서브 journald 시각 `15:48:09`(KST 로 읽으면 06:48:09Z)와 맞물리고, 이어진 시도 6(06:50~08:48Z · 갱신 4회)이 증분 needle 재기동으로 보인다 — 이 둘은 **시각 대조에 의한 추론**이다. 시도 8 창 안에 루트 lite raw(`output/multi/benchlog/lite_raw_solar31-nv4-1m-tp2.json` · measured_utc 2026-10-24T15:16:45Z)가 있다. 시도 1 · 2 · 4 · 9 · 10 이 어느 벽(또는 정상 종료)인지는 로그 · 문서에 대응 기록이 없어 **미확인**이다. 시도 11 이 측정 기동이다(declare 02:43:38Z → API 서버 02:57:07Z · 13분 29초).

### W1 stock vLLM 0.31.0 은 Solar-Open2 아키텍처를 모른다

체크포인트는 `SolarOpen2ForCausalLM` · `model_type: solar_open2` 을 선언한다. stock 0.31.0 불가의 근거는 이미지 안 소스 레지스트리 grep 이다(기동 시도 없이 판정 · 로드 0초). ⚠ 판정 testlog 는 "modeling 코드 없음(trust_remote_code 폴백 불가)" 이라 적었으나 이 저작 때 확인하니 체크포인트 디렉터리에 `modeling_solar_open2.py` 가 실재하고 config `auto_map` 이 그것을 가리킨다 — 그 주장은 틀렸고, trust_remote_code 폴백으로 띄울 수 있는지는 **미시험**이다. HF 모델 카드는 v0.22.0 기반 Upstage 포크를 요구한다. 사람의 결정("2번(자체 이식) 수행")으로 포크 커밋 1개의 델타(20파일 · 실질 ≈2,961줄)를 0.31 API 로 다시 쓴 out-of-tree 플러그인(`vllm.general_plugins` entry point)을 런타임 패치로 넣었다. 0.31 에 맞추며 다시 쓴 파손은 6종이다(FusedMoE→FusedMoEFactory · KDA import 경로 · KDA 레이어 · state 2-튜플 · 로딩 프로토콜 · 파서 import).

> [원문] testlog_26102413_solar_open2_stock_0.31.0_아키텍처_미지원_판정 §① 증상 (이미지 내부 실측)
> - 이미지 `/workspace/vllm-src/vllm/model_executor/models/registry.py` 의 solar 항목은 `SolarForCausalLM` **하나뿐** — `SolarOpen2*` 부재(원문 grep). 파서 `solar_open2`·로짓 프로세서도 부재 추정(포크 소속).

```hint-event
id: W1
kind: wall
증상: stock 0.31.0 레지스트리에 SolarOpen2 모델 · 파서가 없어 기동 불가 판정(기동 시도 없음)
서명: "의 solar 항목은 `SolarForCausalLM` **하나뿐** — `SolarOpen2*` 부재(원문 grep)"
원인: Solar-Open2 지원은 Upstage 포크(v0.22.0 기반 · 커밋 00907fc9b982f973808a0193ad0f8c61fbb62854)에만 있다
해소: 포크 델타를 0.31 API 로 다시 쓴 런타임 플러그인 번들(solar31-nv4-1m-tp2_patch.py · entry point solar_open2 · register 가 config · 모델 · reasoning/tool 파서 등록)
검증: 측정 기동 엔진 로그 Resolved architecture SolarOpen2ForCausalLM(lite_engine_head L68) · serve proof health 200 + 추론 1회
전이등급: arch-invariant
출처: [testlog_26102413_solar_open2_stock_0.31.0_아키텍처_미지원_판정 §① 증상 (이미지 내부 실측), testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §1. 아키텍처 미지원 판정과 이식 경로, solar31-nv4-1m-tp2_patch.py, output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log]
```

### W2 포크의 logits processor 는 0.31 V2 인터페이스가 아니다

첫 기동 구성은 모델 카드대로 `--logits-processors …SolarOpen2TemplateLogitsProcessor` 를 넣었고, 0.31 이 V2 모델러너 인터페이스의 subclass 를 요구해 막혔다. 첫 번째 셀에서는 기동 · 장문과 무관한 템플릿 강제 기능이라 빼고 띄웠고(기능 공백을 후속으로 기록), plan_26102420 S4 에서 슬롯 기반 V2 `LogitsProcessor` 로 다시 써 넣었다.

> [원문] testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌)
> | 1 | `SolarOpen2TemplateLogitsProcessor is not a subclass of vllm.v1.worker.gpu.sample.logits_processor.LogitsProcessor` | 0.31 은 v2 모델러너 인터페이스 요구 · 포크 구현은 v1 | 이 셀에서 **미이식**(템플릿 강제 기능 공백 · 기동·장문과 무관) — 후속 |
> | 2 | slave: `No module named 'solar_open2_plugin'` | (a) sync 스냅샷이 인덱스 권위라 비추적 패치가 서브에 안 감 (b) slave 는 `CONFIG_FILE=default`(NEVER_FORWARD)라 model-keyed 패치를 arming 못 함 | (a) `sync_to_sub.sh` 가 `BAND2_RUNTIME_PATCH_STEMS` 등재 stem 의 패치쌍·모델 입력을 파일시스템에서 스냅샷 편입(추적하면 사이드카 digest 가 단일권위 tripwire 에 걸림) (b) compose `RUNTIME_PATCH_STEM` + `arm_patch.sh` 키 + `slave_forward` tripwire 표(자체검사 PASS) |
> | 3 | 가중치 로딩 `weight_loader() missing 'loaded_shard_id'` | AutoWeightsLoader 는 shard id 를 텐서 속성으로 싣고 2-인자 호출 · 0.31 fused conv1d 로더는 3-인자 | KDA 서브클래스에서 conv1d 로더를 속성 읽기 어댑터로 감쌈 |
> | 4 | 400k 단발 프리필에서 slave 컨테이너 kill(137) | **sub tp-watchdog `gpu_pwr_sustained`**(GPU 81 W · 버킷 60/60 · journald `easy-vllm-bb-tp-watchdog` 15:48:09) — 가드 정상 작동. Nemotron 선례와 동일 | 가드 우회·수정 ✗. 40k→(32k·26k·20k·16k) 증분 프리필로 재시험, 재기동 |

```hint-event
id: W2
kind: wall
증상: logits processor 클래스가 0.31 의 V2 인터페이스 subclass 검사에 걸려 기동 불가
서명: "`SolarOpen2TemplateLogitsProcessor is not a subclass of vllm.v1.worker.gpu.sample.logits_processor.LogitsProcessor`"
원인: 포크 구현은 v1 어댑터 인터페이스이고 0.31 은 V2 모델러너 LogitsProcessor 를 요구한다
해소: 첫 셀은 프로세서를 빼고 기동 · plan_26102420 S4 에서 슬롯 기반 V2 LogitsProcessor 로 재저작해 다시 넣음
검증: 측정 기동 엔진 non-default args 에 logits_processors 가 실리고 서버 기동(lite_engine_head L64 · lite_engine L736)
전이등급: arch-invariant
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌), plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화 §정정·진행 기록 (2026-10-25 KST · S2~S4 완료), output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log]
```

### W3 서브(Ray 워커)에 플러그인이 없다

메인은 패치를 무장했지만 서브의 Ray 워커가 모델 객체를 unpickle 하다 `solar_open2_plugin` 을 못 찾아 액터가 죽었다. 원인은 두 겹이다 — 비추적 패치 파일이 서브 배달 스냅샷에 들지 않았고, slave 는 `CONFIG_FILE=default` 라 모델 키 패치를 찾지 못했다. 교정은 배달 스냅샷 편입 + compose `RUNTIME_PATCH_STEM` 전달 + `arm_patch.sh` 가 그 키를 우선 읽게 한 것이다(커밋 d781f26). 무장은 `[arm_patch] runtime patch armed → /app/configs/solar31-nv4-1m-tp2_patch.py` 줄로 판정한다(교정 뒤 slave 로그 `slave_failure.log` L34 에 그 줄이 있다).

> [원문] output/multi/benchlog/serve_fail_solar31-nv4-1m-tp2/master_exited.log §L177-179
> ray.exceptions.ActorDiedError: The actor died because of an error raised in its creation task, ray::vllm_Worker_1792821943096907668_TP1:RayWorkerProc.__init__() (pid=401, ip=<node:sub>, actor_id=739f6262b86a02a93f45747701000000, repr=<vllm.v1.executor.ray_executor_v2.RayWorkerProc object at 0xe5f8ac7edf10>)
>   At least one of the input arguments for this task could not be computed:
> ray.exceptions.RaySystemError: System error: No module named 'solar_open2_plugin'

```hint-event
id: W3
kind: wall
증상: 서브 TP rank 1 Ray 워커 생성 중 모듈 부재로 액터가 죽어 엔진 기동 실패
서명: "ray.exceptions.RaySystemError: System error: No module named 'solar_open2_plugin'"
원인: 비추적 런타임 패치가 서브 배달 스냅샷에 들지 않았고 slave 는 CONFIG_FILE=default 라 모델 키 패치를 무장하지 못했다
해소: sync_to_sub 스냅샷에 등재 stem 패치쌍 편입 · compose RUNTIME_PATCH_STEM 서브 전달 · arm_patch.sh 가 RUNTIME_PATCH_STEM 을 CONFIG_FILE 보다 먼저 읽음(커밋 d781f26)
검증: 교정 뒤 slave 로그의 arm_patch armed 줄(serve_fail slave_failure.log L34) · 측정 기동에서 서브 rank 가 가중치 적재 완료(lite_engine L700 · L703)
전이등급: judgment
출처: [output/multi/benchlog/serve_fail_solar31-nv4-1m-tp2/master_exited.log, testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌), arm_patch.sh, output/multi/benchlog/serve_fail_solar31-nv4-1m-tp2/slave_failure.log]
```

### W4 conv1d 로더의 shard id 인자 불일치

가중치 적재 중 0.31 의 fused conv1d `weight_loader` 가 `loaded_shard_id` 위치 인자를 요구해 실패했다. AutoWeightsLoader 는 shard id 를 텐서 속성으로 싣고 2-인자로 부르기 때문이다. KDA 서브클래스에서 conv1d 로더를 속성을 읽어 3-인자로 넘기는 어댑터로 감싸 넘었다. 통과 판정은 양 rank 의 `Model loading took 71.6 GiB` 줄이다.

```hint-event
id: W4
kind: wall
증상: 가중치 적재 중 conv1d weight_loader 호출이 인자 부족으로 실패
서명: "가중치 로딩 `weight_loader() missing 'loaded_shard_id'`"
원인: AutoWeightsLoader 는 shard id 를 텐서 속성으로 싣고 2-인자 호출하는데 0.31 fused conv1d 로더는 3-인자다
해소: KDA 서브클래스에서 conv1d 로더를 텐서 속성 읽기 어댑터로 감쌈(플러그인 kda_layer.py)
검증: 측정 기동 lite_engine 로그 L696 · L703 Model loading took 71.6 GiB(양 rank)
전이등급: arch-invariant
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌), solar31-nv4-1m-tp2_patch.py, output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log]
```

### W5 400k 단발 프리필에서 서브 GPU 전력 가드가 slave 를 사살

1차 needle 은 62,929 · 150,929 · 266,129 토큰을 통과한 뒤 400k 단발 프리필에서 slave 컨테이너가 137 로 죽었다. 서브 tp-watchdog `gpu_pwr_sustained`(GPU 81 W · 버킷 60/60)가 정상 작동한 것이다 — 메모리 가드(memwatch)는 TRIP 0 이었다. 가드를 우회 · 수정하지 않고 재기동해 40k 단위 → 32k · 26k · 20k · 16k 단위 증분 프리필로 1,001,129 토큰까지 전 단계 PASS · TRIP 0 으로 올렸다. 이 kill 은 서브 원장 회수 범위(회수 미러 마지막 행 2026-10-24T00:02:35Z) 밖이라 01 §1.4 원장 표에는 나오지 않는다(02 Q7).

```hint-event
id: W5
kind: wall
증상: 400k 토큰 단발 프리필 중 서브 slave 컨테이너가 exit 137 로 사살
서명: "400k 단발 프리필에서 slave 컨테이너 kill(137)"
원인: 장문 프리필 동안 서브 GPU 전력이 tp-watchdog gpu_pwr_sustained 버킷(81 W · 60/60)을 채웠다 — 가드 정상 작동
해소: 가드 우회 · 수정 없이 재기동 후 40k 이하 단위 증분 프리필
검증: testlog_26102418 §4 2차 증분 1,001,129 토큰 PASS · TRIP 0
전이등급: arch-scaled
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌), testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §4. 장문 needle (needle=`MAGNOLIA-7734` · temperature 0)]
```

### W6 reasoning 이 분리되지 않고 본문에 섞인다

첫 구성은 `--default-chat-template-kwargs` 없이 띄웠고, 응답 본문에 "Thinking Process" 가 섞이며 reasoning 필드가 비었다. S4 에서 모델 카드 권장값 `{"think_render_option":"preserved"}` 와 V2 프로세서를 함께 넣어 분리를 완결했다(plan_26102420 정정 기록 · 세부 관측은 계보 경유 문서 testlog_26102500 §4 — 읽기 목록 밖).

```hint-event
id: W6
kind: wall
증상: 응답 content 에 사고 서술이 섞이고 reasoning 필드가 비어 파서 분리가 동작하지 않음
서명: "가 섞이고 reasoning 필드는 비어 있음(파서 분리 미작동 — 후속)"
원인: 미확정 — 실패한 첫 구성은 chat-template kwargs(think_render_option)와 V2 logits processor 가 둘 다 없었고(serve_fail master_exited.log non-default args) S4 는 둘을 함께 넣었다 · 어느 쪽이 필요한지 가르는 단독 대조가 없다
해소: default-chat-template-kwargs think_render_option=preserved + V2 logits processor(plan_26102420 S4)
검증: plan_26102420 §정정·진행 기록 (2026-10-25 KST · S2~S4 완료) 의 reasoning 분리 완결 · 측정 기동 non-default args(lite_engine_head L64)
전이등급: arch-invariant
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌), plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화 §정정·진행 기록 (2026-10-25 KST · S2~S4 완료)]
```

### W7 V2 프로세서는 동기 스케줄링을 요구한다

S4 구성(시도 7)은 가중치를 다 올린 뒤 워커 초기화에서 프로세서의 fail-loud 검사에 걸렸다. yaml 의 `async-scheduling: false` 가 무효였기 때문이다(M1). 러너에 `--no-async-scheduling` 을 넣고 다시 띄웠다.

> [원문] output/multi/benchlog/serve_fail_solar31-nv4-1m-tp2/master_failure.log §L1265-1265
> NotImplementedError: SolarOpen2TemplateLogitsProcessor (V2) needs synchronous scheduling (--no-async-scheduling): with async scheduling the last sampled token is not yet committed when apply() runs

```hint-event
id: W7
kind: wall
증상: 가중치 적재 후 워커 초기화에서 NotImplementedError 로 엔진 기동 실패
서명: "NotImplementedError: SolarOpen2TemplateLogitsProcessor (V2) needs synchronous scheduling (--no-async-scheduling)"
원인: V2 프로세서는 apply 시점에 마지막 샘플 토큰이 확정돼 있어야 하는데 비동기 스케줄링이 켜져 있었다(yaml false 무효 · M1)
해소: 러너 solar31-nv4-1m-tp2.sh 명령줄에 --no-async-scheduling
검증: 측정 기동 non-default args 의 async_scheduling False(lite_engine_head L64) · 서버 기동(lite_engine L736)
전이등급: arch-invariant
출처: [output/multi/benchlog/serve_fail_solar31-nv4-1m-tp2/master_failure.log, solar31-nv4-1m-tp2.sh, output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log]
```

### W8 bench 예산 4096 MiB 선언이 `floor − bench ≥ abs_band` 게이트에 막힌다

full 벤치 클라이언트(GuideLLM 컨테이너)의 메모리 예산을 4096 MiB 로 선언하자 BLOCK 됐다. testlog 는 "arm_ceiling 한도 초과" 라 적었지만 4096 은 arm_ceiling 9296 MiB 보다 작다 — 실제 게이트는 `바닥 − 벤치 예산 ≥ 절대밴드`(budget_preflight.py 머리 주석 · 같은 계산을 이 셀 선언값으로 돌리면 12368 − 4096 = 8272 < 10240 → 불가 · 벤치 예산 상한 2128 MiB)다. 2048 MiB(12368 − 2048 = 10320 ≥ 10240)로 선언해 측정했다.

```hint-event
id: W8
kind: wall
증상: full 벤치 예산 선언이 BLOCK
서명: "bench 예산 4096 MiB 가 arm_ceiling 한도 초과로 BLOCK(Qwen TP1 9527 · Solar multi 9296 MiB)"
원인: 벤치 진입 게이트 floor − bench_budget ≥ abs_band(10240 MiB)를 어겼다 — 이 셀 floor 12368 MiB 에서 벤치 예산 상한은 2128 MiB 다(arm_ceiling 대비가 아니다)
해소: bench 예산 2048 MiB 로 선언
검증: testlog_26102504 §2 표 2번 행 처방 · 측정 완주(bench_report 반복 3/3)
전이등급: arch-scaled
출처: [testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2. 하네스 결함 · 처방, docs/logs/main/events/watchdog.jsonl]
```

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: 모델 카드가 요구하는 Upstage 포크 UpstageAI/vllm 브랜치 v0.22.0-solar-open2(커밋 00907fc9b982f973808a0193ad0f8c61fbb62854)를 빌드 좌표로 핀(사다리의 포크 핀 칸)
기각사유: 포크 베이스가 v0.22.0 이라 0.31.0 이미지에서 포크로 가면 vLLM 다운그레이드가 된다 — 자체 이식 선판정 3신호가 go 로 나와 포크 핀 앞 칸(자체 이식)에서 멈췄다
재개조건: 자체 이식본이 수치 대조(02 Q1)에서 틀린 것으로 드러나고 0.22.0 계열로 내려가도 되는 셀일 때
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §1. 아키텍처 미지원 판정과 이식 경로, testlog_26102413_solar_open2_stock_0.31.0_아키텍처_미지원_판정 §처분]
```

```hint-event
id: R2
kind: rejected
시도: 이식물을 빌드 패치(소스에 구워 넣는 pre 슬롯)로 이미지에 넣기
기각사유: 이식물이 순수 Python 이라 빌드 시점 성립이 필요 없고, 이미지 하나가 모든 모델을 서빙하는 불변식에서 모델 전용 코드를 이미지에 넣으면 분기 압력이 생긴다 — +1 런타임 패치 슬롯으로 배정(이미지 불변)
출처: [plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화 §슬롯 판정 (자율 판단 · 근거), testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §1. 아키텍처 미지원 판정과 이식 경로]
```

```hint-event
id: R3
kind: rejected
시도: 모델 카드 서빙 인자 --moe-backend triton
기각사유: 0.31 의 NVFP4 MoE 백엔드 후보 목록에 TRITON 이 없다(엔진 로그 후보 FLASHINFER_TRTLLM · FLASHINFER_CUTEDSL · FLASHINFER_CUTEDSL_BATCHED · FLASHINFER_CUTLASS · VLLM_CUTLASS · MARLIN · HUMMING · EMULATION) — marlin 으로 처음부터 띄웠다(triton 으로 기동해 실패한 기록은 없다 · 후보 목록에 없어 시도하지 않았다)
재개조건: 대상 vLLM 의 NvFp4 MoE 후보 목록에 TRITON 이 나타날 때
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌), output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log]
```

```hint-event
id: R4
kind: rejected
시도: 싱글 노드 TP=1 로 Solar NVFP4 기동
기각사유: 체크포인트 142.76 GiB 가 노드 1대 통합메모리를 넘는다는 산술 사전판정으로 기동 없이 멀티로 이관(시도 0 — 기동 실패가 아니라 산술 판정)
출처: [testlog_26102418_4모델_싱글멀티_연속캠페인_결과요약_종결 §결과표, testlog_26102412_4model_싱글캠페인_릴레이요약_종결 §이관]
```

```hint-event
id: R5
kind: rejected
시도: sweep_bench.sh 를 도구 지정 없이 돌린 첫 full 패스(기본 도구 vLLM bench)
기각사유: full 정의(GuideLLM × 반복 ≥3)를 충족하지 않는 도구로 측정돼 무효 — 구 리포트 26102505_* · 26102506_* 폐기 · --tool guidellm 으로 전 셀 재측정
출처: [testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2. 하네스 결함 · 처방, devlog_26102504_PLE_mmap_재이식_full벤치_hint발행_서사 §시도 · 폐기]
```

```hint-event
id: R6
kind: rejected
시도: 장문 프리필 중 서브 tp-watchdog 사살을 피하려고 가드를 우회하거나 임계를 고치기
기각사유: 가드는 정상 작동이었다(81 W · 버킷 60/60) — 우회 · 수정 대신 40k→32k·26k·20k·16k 증분 프리필로 재시험해 1,001,129 토큰까지 TRIP 0
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌), plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화 §안전 경계]
```

```hint-event
id: R7
kind: rejected
시도: speculative decoding(MTP 등) 켜기
기각사유: 이식한 V2 logits processor 와 KDA 레이어가 spec decode 를 받지 않는다 — 번들 소스상 V2 프로세서 __init__ 이 speculative_config 가 있으면 NotImplementedError 로 기동을 막고(logits_processor.py), 프로세서를 뺀 구성이면 KDA 레이어가 forward 중 spec 메타데이터를 만나 raise 한다(kda_layer.py · 기동 검사 아님) — 01 §1.2 의 해제 명령으로 볼 수 있다. 실제로 켜서 실패한 시도는 계보에 없다
재개조건: 프로세서 · KDA 가 요청당 여러 logits 행과 spec 메타데이터를 처리하도록 다시 이식될 때
출처: [solar31-nv4-1m-tp2_patch.py]
```

**요약** — 이 셀의 형상을 좁힌 것은 세 갈래다. ① 경로: stock 미지원 → 포크 핀(다운그레이드) 대신 자체 이식, 빌드 패치 대신 런타임 플러그인(R1 · R2). ② 형상: 싱글은 산술로 배제(R4), MoE 는 0.31 후보 목록에 있는 marlin(R3), spec decode 는 이식본이 받지 않는다(R7). ③ 검증 · 측정 절차: 장문은 가드 우회가 아니라 증분(R6), 측정은 GuideLLM 재측정분만 유효(R5).

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

### M1 yaml 의 `async-scheduling: false` 가 동기 스케줄링을 건다는 믿음

S4 에서 V2 logits processor 를 넣고 띄운 시도(01 §1.4 기동 시도 7)는 가중치 적재 뒤 워커 초기화에서 `NotImplementedError` 로 멈췄다 — 프로세서가 동기 스케줄링을 요구하는데 엔진은 비동기 스케줄링으로 resolve 돼 있었다. yaml 에 적힌 `async-scheduling: false` 는 엔진 설정 파서가 불리언 false 를 버려 효과가 없었다(지금 yaml 의 그 줄은 주석으로 남겨 무효를 표시한다). 교정은 러너 `.sh` 의 명령줄 `--no-async-scheduling` 이고, 측정 기동의 엔진 `non-default args` 에 `'async_scheduling': False` 가 찍혔다(lite_engine_head 로그 L64).

```hint-event
id: M1
kind: misdiagnosis
증상: yaml 에 async-scheduling false 를 두었는데 V2 logits processor 가 비동기 스케줄링이라며 기동을 막았다
서명: "async-scheduling: false  ← 무효(설정 파서가 false 를 버림)"
원인: vLLM 설정 파일 파서가 불리언 false 값을 버려 non-default 인자에 들어가지 않았고 비동기 스케줄링이 기본값으로 켜졌다
해소: 러너 명령줄에 --no-async-scheduling 을 명시 · 엔진 non-default args 의 async_scheduling False 로 적용을 확인
검증: output/multi/benchlog/serve_fail_solar31-nv4-1m-tp2/master_failure.log L1265(교정 전 NotImplementedError) · lite_engine_head 로그 L64(교정 후 async_scheduling False)
전이등급: arch-invariant
현재지위: 반증
출처: [solar31-nv4-1m-tp2.yaml, solar31-nv4-1m-tp2.sh, output/multi/benchlog/serve_fail_solar31-nv4-1m-tp2/master_failure.log, output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log]
```

### M2 실패한 sweep 뒤에 나온 PASS

full 벤치 중 sweep 이 rc=3 으로 실패했는데 판정기 `judge_bench.sh` 가 과거 산출물로 PASS 를 냈다. 이 셀(Solar)에서 실제로 일어났고, 그 판정은 폐기하고 재측정했다 — 03 의 PASS 는 재측정분이다. 판정기 쪽 처방(sweep rc · 신선도 검사)은 미해결이다(02 Q8).

> [원문] testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2. 하네스 결함 · 처방
> | 4 | `judge_bench.sh` 가 실패한 sweep(rc=3) 뒤에도 과거 산출물로 PASS 를 냄(Solar 에서 실제 발생 — 폐기하고 재측정) | **미해결 하네스 결함** — 판정기가 sweep rc/신선도를 검사해야 함. 드라이버에서 sweep rc≠0 이면 judge 건너뛰도록 보강 필요 |

```hint-event
id: M2
kind: misdiagnosis
증상: sweep 실패(rc=3) 뒤 판정 PASS 가 나왔다
서명: "가 실패한 sweep(rc=3) 뒤에도 과거 산출물로 PASS 를 냄(Solar 에서 실제 발생 — 폐기하고 재측정)"
원인: 판정기가 sweep 의 종료코드와 산출물 신선도를 검사하지 않고 디렉터리에 남은 과거 산출물을 읽었다
해소: 그 PASS 를 폐기하고 같은 셀을 다시 측정 · 03 의 판정은 재측정분(verdict.json · sweep_index generated_utc 2026-10-25T03:13:30Z)
검증: testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2 표 4번 행 · bench_report_26102512 생성일
전이등급: judgment
현재지위: 반증
출처: [testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2. 하네스 결함 · 처방, bench_report_26102512_solar-open2-250b-nota-nvfp4_GB10_0.31.0 §판정 (표시만 — verdict_rule.py 결과)]
```

### M3 체크포인트 크기를 `du` 로 잰 사전 가정

첫 plan(plan_26102401)은 NAS `du` 286G 를 근거로 "멀티 TP=2 에서도 가중치 143G/노드" 일 수 있다고 가정하고 CPU offload 필요성을 판정 대상으로 올렸다. 뒤의 plan(plan_26102412)이 safetensors 실측 total_size 142.76 GiB → 노드당 ≈71.4 GiB 로 정정했고, 측정 기동의 엔진도 `Checkpoint size: 142.78 GiB` 와 노드당 적재 71.6 GiB 를 찍었다. offload 는 필요하지 않았다.

```hint-event
id: M3
kind: misdiagnosis
증상: NAS du 가 Solar NVFP4 를 286G 로 보고했다
서명: "멀티 TP=2 에서도 가중치 143G/노드 > 121GiB 이면 CPU offload/PLE mmap 필요 여부를 판정"
원인: 파일시스템 du 는 이 체크포인트의 실제 가중치 바이트보다 크게 나왔다(du 가 왜 부풀었는지는 계보에 기록 없음)
해소: safetensors total_size 142.76 GiB(전체) → 노드당 ≈71.4 GiB · 엔진 실측 노드당 71.6 GiB · offload 불요
검증: plan_26102412 §셀 · testlog_26102412 §이관(du 286G 는 부풀려짐) · lite_engine 로그 L546 · L696 · L703
전이등급: arch-invariant
현재지위: 반증
출처: [plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증 §⚠ 사전에 짚을 전제 (착수 전 확증 대상), plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context §셀 (TP=2 이므로 노드 병렬 아님 · 순차), testlog_26102412_4model_싱글캠페인_릴레이요약_종결 §이관]
```

**교정 묶음**
- testlog_26102504 §2(항목 1~5 · 5개 검토) 중 이 셀에 영향을 준 것:
  - testlog_26102504 1 — 측정 도구 — vLLM bench(첫 패스 · 무효 · 구 리포트 `26102505_*` `26102506_*`) → GuideLLM 0.7.3 재측정(R5).
  - testlog_26102504 2 — bench 예산 선언 — 4096 MiB(BLOCK · 실제 게이트 floor − bench ≥ abs_band · 상한 2128 MiB) → 2048 MiB(W8).
  - testlog_26102504 4 — 판정 — 과거 산출물 PASS → 폐기 · 재측정 PASS(M2 · 판정기 결함 자체는 미해결).
  - 3(Qwen 싱글 E 초과)과 5(서브 러너 사다리)는 이 셀의 판정 · 수치를 바꾸지 않았다.
- testlog_26102418_4모델 §하네스 교정(항목 1~4 · 4개 검토) 중 이 셀에 영향을 준 것:
  - 3 — 런타임 패치 → slave 경로 — 서브에 패치 부재로 기동 실패 → `RUNTIME_PATCH_STEM` 배선 + 스냅샷 편입(W3 · 커밋 d781f26).
  - 4 — 멀티 서브 통로 · 승인 앵커 실주행 — 절차 변경이며 이 셀의 수치 · 판정은 바꾸지 않았다.
  - 1(싱글 yaml 포트)과 2(YaRN 번역)는 이 셀과 무관하다(이 셀은 YaRN 을 쓰지 않는다).

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

**토폴로지 · 병렬(V1 · V2 · V3)** — 처음부터 멀티 TP=2 · Ray · expert-parallel 이다. 싱글 TP=1 은 체크포인트 142.76 GiB 가 노드 1대를 넘는다는 **산술 사전판정**으로 기동 없이 이관됐고(시도 0 — 관측된 실패가 아니다), expert-parallel 은 HF 모델 카드의 서빙 인자를 옮긴 것이다(testlog_26102413 §② 외부 확증). 계보에서 다른 TP · EP 값을 시도한 기록은 없다.

**컨텍스트(V6)** — 1,048,576 은 체크포인트 `max_position_embeddings` 그대로다(rope_scaling 없음 · use_rope false → YaRN 주입 없음). 목표가 "지원 최대치 기동"이었으므로 값은 처음부터 끝까지 같다. 기동 검증은 이전 구성에서 1,001,129 토큰 needle 까지 올렸다(02 Q2 — 측정 구성에서는 다시 올리지 않았다).

**KV(V7 · V8)** — fp8_e4m3 · 절대 클램프 15032385536 바이트(14 GiB)는 계보 내내 바뀌지 않았다(KV 스윕 기록 없음 → 승계값). 이 클램프에서 엔진이 잡은 용량은 1M 요청 1개의 1.15배다 — 필요량(1M 토큰 1개)을 조금 넘게 잡은 값이지 음성대조용 과잉이 아니다. 엔진은 KDA 의 mamba 상태 페이지와 attention 페이지를 맞추려고 블록 크기를 2128 토큰으로 올렸다(아래 발췌 다음 줄 · 엔진 로그 L698).

> [원문] output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log §L703-703
> (RayWorkerProc pid=403, ip=<node:sub>) (Worker_TP1_EP1 pid=403) INFO 10-25 02:55:10 [model_runner.py:407] Model loading took 71.6 GiB memory and 619.685076 seconds

> [원문] output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log §L710-710
> (EngineCore pid=986) INFO 10-25 02:56:10 [kv_cache_utils.py:2464] GPU KV cache size: 1,206,177 tokens, Maximum concurrency for 1,048,576 tokens per request: 1.15x

해설: 앞 줄은 서브(TP rank 1)의 적재 실측(71.6 GiB · 619.7 s · 메인 rank 0 은 593.0 s · L696)이고, 뒷 줄이 이 클램프에서 엔진이 잡은 KV 용량이다.

**호스트 메모리 예산(선언)** — 측정 기동의 예산 선언 행은 아래와 같다. 이 값은 **선언**이다(측정 아님): weights_mib 73104 는 엔진이 잰 노드당 적재 71.6 GiB 와 나란히 놓을 선언값이고, mem_total_mib 124608 은 계보 plan 이 적은 "노드당 통합메모리 ~121GiB" 와 같은 축의 선언이다. 11번의 기동 시도 모두 같은 선언값(floor 12368 MiB)을 썼다(01 §1.4 원장 표).

> [원문] docs/logs/main/events/2026-10.jsonl §L654-654
> {"expires_epoch": 1792903418, "floor_mib": 12368, "kind": "budget_declare", "kv_mib": 14336, "label": "smoke-solar31-nv4-1m-tp2", "mem_total_mib": 124608, "overhead_mib": 24800, "source": "blackbox_session", "ts": "2026-10-25T02:43:38Z", "weights_mib": 73104}

관측 쪽은 측정 중 시스템 RAM 점유 main 101.2 GiB(83%) · sub 99.3 GiB(82%)다(bench_report §lite 지표).

**실행 모드 · 배치(V9 · V10 · V11)** — enforce-eager true · max-num-seqs 1 은 첫 셀 testlog 유효맥락부터 측정까지 같고, max-num-batched-tokens 2048(과 gpu-memory-utilization 0.80 · V5)은 계보 문서에 다른 값 기록이 없다. eager 를 고른 이유(캡처 스파이크 실측 등)는 계보에 적혀 있지 않고 cudagraph 시도는 0 이다(02 Q5) — 승계값이다. max-num-batched-tokens 2048 도 바꿔 잰 기록이 없다.

**MoE 백엔드(V4)** — 모델 카드는 `--moe-backend triton` 을 적었으나 0.31 의 NVFP4 MoE 후보 목록에 TRITON 이 없어 marlin 을 처음부터 썼다(R3). 엔진은 Marlin 경로에서 FP4 를 weight-only 로 처리한다는 경고를 낸다(lite_engine L694 — 이 경고는 GB10 의 FP4 지원 여부를 판별한 근거로 읽지 않는다 · MARLIN 은 yaml 노브 선택의 결과다). 다른 후보(FLASHINFER_* · VLLM_CUTLASS · HUMMING)는 시도 0 이다.

**파서 · 프로세서 · 템플릿(V12~V16)** — reasoning/tool 파서와 auto-tool-choice 는 모델 카드 인자 그대로다. logits-processors 는 처음엔 v1 구현이라 기동이 막혀(W2) 이 셀에서 빼고 띄웠다가, plan_26102420 S4 에서 V2 로 다시 써 넣었다. default-chat-template-kwargs 는 첫 구성에 없었고 그 구성에서 reasoning 분리가 동작하지 않았다(W6 — 프로세서도 함께 없던 구성이라 단독 원인은 미확정). S4 에서 모델 카드 권장값 `think_render_option=preserved` 를 프로세서와 함께 넣었다. 이 두 값을 넣으면서 러너에 `--no-async-scheduling` 이 필요해졌다(W7 · M1).

**버티는 기전 대 다른 형상**
- 싱글 TP=1(같은 NVFP4 · 1M) — 노드 1대 메모리 대 체크포인트 크기(판정 당시 입력 du 286G · 뒤 기재 142.76 GiB) — 가중치가 통합메모리(~121GiB)에 들어가지 않는다는 산술 — 미결(산술 사전판정 · 기동 대조 없음) — testlog_26102418_4모델 §결과표 · plan_26102401 §⚠ 사전에 짚을 전제.
- 400k 단발 프리필(같은 형상) 대 40k 이하 증분 — 프리필 길이만 다르다 — 단발 장문 프리필 중 서브 GPU 가 81 W · 버킷 60/60 으로 tp-watchdog `gpu_pwr_sustained`(80 W/60 s 버킷 — plan_26102412 §셀) 에 걸려 slave 가 사살됐다 · 증분은 전 단계 TRIP 0 — 확정(가드 사살 기록과 증분 재시험 대조) — testlog_26102418 §2 · §4.
- 이전 구성(logits processor 없음 · chat-template kwargs 없음 · 러너 async 플래그 없음) 대 측정 구성 — 세 설정이 함께 바뀌었다 — 이전 구성도 기동 · 1M needle · lite 는 통과했다(기능 차이는 reasoning 분리 · 템플릿 강제뿐) · lite 차이의 기전은 미결 — 미결 — testlog_26102418 §5 · 02 Q6.

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- stock vLLM 0.31.0 에 `--model` 만 바꿔 Solar-Open2 를 띄우려 하지 말 것 — W1(레지스트리에 SolarOpen2 부재 · 체크포인트의 modeling_solar_open2.py 로 trust_remote_code 폴백이 되는지는 미시험) — 대상 버전 registry.py 를 먼저 grep 하고, 없으면 런타임 플러그인(01 §1.2)이나 정식 지원 버전으로 간다.
- 포크 v0.22.0 의 logits processor 를 그대로 0.31 에 얹지 말 것 — W2(v1 인터페이스는 0.31 V2 모델러너에서 subclass 검사에 걸린다) — 슬롯 기반 V2 `LogitsProcessor` 로 다시 쓴다.
- 런타임 패치를 메인 컨테이너에만 무장하고 끝내지 말 것 — W3(Ray 워커가 모델 클래스를 unpickle 하므로 서브에서 `No module named 'solar_open2_plugin'`) — 서브에도 패치 파일이 같은 경로에 있고 `RUNTIME_PATCH_STEM` 이 서브로 전달되는지 확인한다(arm_patch.sh 의 `[arm_patch] runtime patch armed` 줄이 양 노드 로그에 있어야 한다).
- yaml 에 `async-scheduling: false` 를 적고 동기 스케줄링이 걸렸다고 믿지 말 것 — M1 · W7(설정 파서가 불리언 false 를 버려 V2 프로세서가 `NotImplementedError` 로 기동을 막았다) — 러너 명령줄에 `--no-async-scheduling` 을 두고 엔진 `non-default args` 에 `'async_scheduling': False` 가 찍혔는지 본다.
- 장문 검증을 400k 단발 프리필로 하지 말 것 — W5(서브 tp-watchdog `gpu_pwr_sustained` 가 slave 를 사살) — 가드는 우회 · 수정하지 않고 40k 이하 증분 프리필로 올린다.
- bench 예산을 4096 MiB 로 선언하지 말 것 — W8(벤치 게이트 `floor − bench ≥ abs_band` 위반 · arm_ceiling 대조가 아니다) — `budget_preflight.py` 로 이 구성의 `bench_budget_max_mib`(이 셀 floor 12368 MiB 에서 2128 MiB)를 먼저 구하고 그 이하(이 셀은 2048 MiB)로 선언한다.
- `sweep_bench.sh` 를 `--tool` 없이 돌려 full 이라 부르지 말 것 — R5(기본 도구가 vllm 이라 첫 패스가 무효) — `--tool guidellm` 을 명시하고 리포트 측정 구성 표의 bench_tool 을 확인한다.
- 실패한 sweep 뒤에 나온 PASS 를 믿지 말 것 — M2(판정기가 과거 산출물로 PASS 를 냈다 · 미해결) — sweep rc 와 verdict.json · sweep_index 의 시각이 이번 측정인지 대조한다.
- NAS 의 `du` 로 체크포인트 크기를 재지 말 것 — M3(du 286G 대 safetensors total_size 142.76 GiB) — `model.safetensors.index.json` 의 `metadata.total_size`(config.json 에는 없다)나 엔진 로그 `Checkpoint size` 줄로 잰다.
- 모델 카드의 `--moe-backend triton` 을 그대로 옮기지 말 것 — R3(0.31 NVFP4 MoE 후보 목록에 TRITON 이 없다) — 엔진 로그의 `NvFp4 MoE backend out of potential backends` 줄로 후보를 보고 고른다.
- 엔진 기동 배너의 `0.31.1.dev0+gdb9527a46.d20261014` 를 빌드 입력으로 적지 말 것 — 00 §0.4(빌드 입력은 VLLM_REF=v0.31.0 · 배너는 소스 트리의 개발 버전 문자열) — 재현 좌표는 릴리스 태그 v0.31.0(또는 40자 SHA)이다.
- 이전 Solar 측정(lite 18.25 t/s · 1M needle)을 이 구성의 근거로 옮기지 말 것 — Q2 · Q6(구성이 바뀐 뒤 다시 재지 않았다) — 측정 구성 그대로 다시 잰다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

아래 물음들은 계보 문서와 측정 산출물이 아직 답하지 못한 것이다. 셋(Q1 · Q2 · Q3)은 이 지도를 다른 환경으로 옮길 때 가장 먼저 걸리는 것이다.

```hint-event
id: Q1
kind: open-question
물음: 자체 이식한 KDA(선형 attention) 수치가 HF 레퍼런스 구현과 같은가
현재상태: HF 레퍼런스 대조를 하지 못했다 — 계보 문서는 그 이유를 '레퍼런스 modeling 파일 NAS 부재' 로 적었으나 체크포인트 디렉터리에 HF 레퍼런스 구현 modeling_solar_open2.py 가 실재한다(저작 시 확인 · 이유 진술이 틀렸다). 간접 검증만 있다 — 이전 구성에서 1,001,129 토큰 needle PASS · 한국어/수학 짧은 질의 정상(testlog_26102418 §4). 번들의 model.py 머리 주석도 DRAFT / UNTESTED 라고 적는다
다음관측: 체크포인트의 HF 레퍼런스(modeling_solar_open2.py · transformers + fla) 또는 포크 v0.22.0-solar-open2 이미지로 같은 프롬프트의 prompt_logprobs · 층 출력을 받아 이 이식본과 비교
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §6, solar31-nv4-1m-tp2_patch.py]
```

```hint-event
id: Q2
kind: open-question
물음: 측정한 구성(V2 logits processor · 동기 스케줄링 · think_render_option=preserved)에서 1M 근접 길이가 다시 검증됐는가
현재상태: 1,001,129 토큰 needle PASS 는 V2 이식 전 구성(logits processor 미적용 · chat-template kwargs 미적용 · --no-async-scheduling 명시 전)의 기록이다. 측정 구성의 needle 은 계보 경유 문서(testlog_26102500 · 읽기 목록 밖)에 38k→228k 6/6 PASS 로만 적혀 있다
다음관측: 측정 구성 그대로 40k 증분 needle 을 1M 근처까지 다시 올린다(tp-watchdog 버킷 준수)
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §4, plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화 §정정·진행 기록 (2026-10-25 KST · S2~S4 완료)]
```

```hint-event
id: Q3
kind: open-question
물음: 상위 vLLM 버전이 Solar-Open2 를 정식 지원하게 되면 이 런타임 플러그인이 불필요해지는가
현재상태: stock 0.31.0 레지스트리에 SolarOpen2 항목이 없다(이미지 안 registry.py grep). 상위 버전의 머지 여부는 이 계보에서 조사되지 않았다
다음관측: 대상 버전 소스의 vllm/model_executor/models/registry.py 에서 SolarOpen2ForCausalLM 과 reasoning/tool 파서 solar_open2 등록 여부를 grep
출처: [testlog_26102413_solar_open2_stock_0.31.0_아키텍처_미지원_판정 §① 증상 (이미지 내부 실측)]
```

```hint-event
id: Q4
kind: open-question
물음: 동시성 1 을 넘는 부하에서 이 형상은 어떻게 되는가
현재상태: max-num-seqs 1 · 부하 레벨 1개(동시성 1 · 16 요청 · decode 16.67 t/s)만 쟀다. 엔진 로그의 KV 용량은 1M 요청 1개의 1.15배다
다음관측: max-num-seqs 와 KV 클램프를 다시 도출한 뒤 동시성 2 이상 레벨을 GuideLLM 으로 추가 측정
출처: [bench_report_26102512_solar-open2-250b-nota-nvfp4_GB10_0.31.0 §부하 스윕 곡선 (client-load · reload 없음), output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_solar31-nv4-1m-tp2.log]
```

```hint-event
id: Q5
kind: open-question
물음: eager 대신 cudagraph 로 띄우면 이 플러그인이 동작하며 decode 가 달라지는가
현재상태: enforce-eager true 는 계보 내내 바뀐 적 없다(시도 0). 엔진 로그는 eager 때문에 torch.compile · CUDAGraphs 를 끈다고 적는다
다음관측: 같은 구성에서 enforce-eager 만 빼고 기동 · 캡처 메모리 스파이크와 decode t/s 를 관측
출처: [output/multi/benchlog/sweep_solar31-nv4-1m-tp2/lite_engine_head_solar31-nv4-1m-tp2.log, artifacts/triplet/solar31-nv4-1m-tp2.yaml]
```

```hint-event
id: Q6
kind: open-question
물음: lite warm gen 이 이전 구성 18.25 t/s 에서 측정 구성 17.39 t/s 로 낮아진 원인은 무엇인가
현재상태: 두 lite 사이에 logits processor 적용 · 동기 스케줄링 · chat-template kwargs 가 함께 바뀌었다. 어느 변경이 기여했는지 가르는 대조가 없다(계보 경유 문서 testlog_26102500 은 그 사이 17.42 t/s 를 노이즈 범위라 적었으나 근거 밴드를 적지 않았다)
다음관측: 같은 이미지에서 logits processor 만 끈 기동(--no-async-scheduling 유지)으로 lite 를 재측정
출처: [testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §5. lite, bench_report_26102512_solar-open2-250b-nota-nvfp4_GB10_0.31.0 §lite 지표 (full ⊇ lite — 열 집합 중첩 보장)]
```

```hint-event
id: Q7
kind: open-question
물음: 서브 노드의 빌드 원장 ABI 축과 측정 시점 서브 원장 사건은 무엇이었나
현재상태: attestation 의 vllm_sha · torch · driver 는 양 노드 같음이지만 build_ledger 축은 sub_identity_sha256 이 비어 unobservable 이다. 서브 블랙박스 원장의 회수 미러는 2026-10-24T00:02:35Z 에서 끝나 이 셀 기동 · 측정 창의 서브 사건(예산 · 사살)은 관측 범위 밖이다 — 400k 트립도 서브 journald 를 인용한 testlog 서술로만 남아 있다
다음관측: 서브 원장 회수(문서 기반 상향) 후 측정 창 행 확인 · 서브 이미지 빌드 원장 identity 를 attestation 에 싣기
출처: [output/multi/benchlog/attestation_solar31-nv4-1m-tp2.json, testlog_26102418_solar_open2_nvfp4_멀티_TP2_1M_자체이식_기동_needle_lite_판정 §2. 기동까지의 결함 4건(전부 라이브가 알려줌)]
```

```hint-event
id: Q8
kind: open-question
물음: 판정기 judge_bench 의 신선도 결함이 고쳐졌는가
현재상태: 실패한 sweep(rc=3) 뒤 과거 산출물로 PASS 를 낸 사례가 이 셀에서 실제로 있었고 폐기 후 재측정했다. 처방(sweep rc · 신선도 검사)은 미해결로 남아 있다
다음관측: 판정기에 sweep rc≠0 이면 판정을 건너뛰는 가드가 들어갔는지 확인
출처: [testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2. 하네스 결함 · 처방]
```

- 현행 full 정의(03 §3.5 사실 블록)로 보면 이 측정은 **충족**이다(GuideLLM 0.7.3 · 판정점 반복 3) — 열린 물음이 아니다. 다만 판정 권한은 weak(외부 레퍼런스 없음 · roofline-only)이라 인증서가 없다.
- 엔진 로그의 `Auto-initialization of reasoning token IDs failed` 경고는 reasoning 분리 실패를 뜻하는지 이 계보에서 대조되지 않았다 — 계보 경유 문서(testlog_26102500 §4 · 읽기 목록 밖)는 같은 구성에서 `message.reasoning` 분리가 동작했다고 적는다.
