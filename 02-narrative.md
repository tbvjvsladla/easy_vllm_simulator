# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 79건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/testlog/testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함.md` | testlog | 26102504 | 0 | 3189 | 0.31.0 · 1m · easy-vllm:0.31.0-cu133-aarch64-source · nv4 · q38fn31 · q38fn31-nv4-1m-tp2 · tp2 | — |
| 2 | `docs/plan/plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context.md` | plan | 26102412 | 0 | 2576 | 0.31.0 · 1m · nv4 · tp2 | — |
| 3 | `docs/plan/plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화.md` | plan | 26102420 | 0 | 6650 | 0.31.0 · 1m · nv4 · tp2 | — |
| 4 | `docs/devlog/devlog_26091009_qwen38fn_24셀_캠페인종합_서사.md` | devlog | 26091009 | 0 | 3411 | 1m · nv4 | — |
| 5 | `docs/testlog/testlog_26091009_qwen38fn_24셀_판정.md` | testlog | 26091009 | 0 | 5466 | 1m · nv4 | docs/devlog/devlog_26091109_하네스교정_R0R10_서사.md · docs/plan/plan_26091108_캠페인재수행_선행_하네스교정.md · docs/testlog/testlog_26091109_하네스교정_R0R10_역채점_음성대조.md |
| 6 | `docs/plan/plan_26091108_qwen38fn_22셀_재수행_캠페인.md` | plan | 26091108 | 0 | 12790 | 1m · nv4 | — |
| 7 | `docs/devlog/devlog_26091114_qwen38fn_22셀재수행_7셀체크포인트_서사.md` | devlog | 26091114 | 0 | 5439 | 1m · nv4 | — |
| 8 | `docs/devlog/devlog_26091117_qwen38fn_22셀재수행_512k그룹완료체크포인트_서사.md` | devlog | 26091117 | 0 | 4918 | 1m · nv4 | — |
| 9 | `docs/devlog/devlog_26091121_qwen38fn_22셀재수행_캠페인완주_서사.md` | devlog | 26091121 | 0 | 3470 | 1m · nv4 | — |
| 10 | `docs/testlog/testlog_26091121_qwen38fn_22셀재수행_캠페인완주_판정.md` | testlog | 26091121 | 0 | 5305 | 1m · nv4 | — |
| 11 | `docs/plan/plan_26091216_qwen38fn_res최소조건_후속캠페인.md` | plan | 26091216 | 0 | 12770 | 1m · nv4 | — |
| 12 | `docs/plan/plan_26092119_hint_publisher_재구성_계보서사_셀단위발행_E2E.md` | plan | 26092119 | 0 | 56993 | 1m · nv4 | — |
| 13 | `docs/devlog/devlog_26102504_PLE_mmap_재이식_full벤치_hint발행_서사.md` | devlog | 26102504 | 0 | 1524 | 0.31.0 · easy-vllm:0.31.0-cu133-aarch64-source | — |
| 14 | `docs/benchmark/bench_report_26102511_qwen3.8-flash-next-nvfp4_GB10_0.31.0.md` | bench_report | 26102511 | 0 | 6160 | 0.31.0 · easy-vllm:0.31.0-cu133-aarch64-source | — |
| 15 | `docs/plan/plan_26090918_qwen38_flashnext_multi_딥캠페인.md` | plan | 26090918 | 0 | 10709 | 1m | — |
| 16 | `docs/testlog/testlog_26091114_qwen38fn_22셀재수행_7셀체크포인트_판정.md` | testlog | 26091114 | 0 | 4306 | nv4 | — |
| 17 | `docs/testlog/testlog_26091117_qwen38fn_22셀재수행_512k그룹완료체크포인트_판정.md` | testlog | 26091117 | 0 | 5417 | nv4 | — |
| 18 | `docs/devlog/devlog_26091304_qwen38fn_res최소조건_후속캠페인.md` | devlog | 26091304 | 0 | 5728 | nv4 | — |
| 19 | `docs/testlog/testlog_26091304_qwen38fn_res최소조건_262k재발행_판정.md` | testlog | 26091304 | 0 | 10341 | nv4 | — |
| 20 | `docs/devlog/devlog_26092311_hint_publisher_G3_D1발행_D2_서사.md` | devlog | 26092311 | 0 | 4280 | nv4 | — |
| 21 | `docs/testlog/testlog_26092311_hint_publisher_G3_D2_lite_판정.md` | testlog | 26092311 | 0 | 4512 | nv4 | — |
| 22 | `docs/testlog/testlog_26092317_native_N1_분산서빙_판정.md` | testlog | 26092317 | 0 | 4848 | nv4 | — |
| 23 | `docs/devlog/devlog_26101017_knob프리셋_E2E_셀M1_q38fn030_nv4_res_262k.md` | devlog | 26101017 | 0 | 1762 | nv4 | — |
| 24 | `docs/plan/plan_26101017_knob_E2E_hint발행.md` | plan | 26101017 | 0 | 3616 | nv4 | — |
| 25 | `docs/testlog/testlog_26101017_knob프리셋_E2E_셀M1_q38fn030_nv4_res_262k_판정.md` | testlog | 26101017 | 0 | 3074 | nv4 | — |
| 26 | `docs/plan/plan_26101104_hint발행체제_개편_서브자체발행.md` | plan | 26101104 | 0 | 11774 | nv4 | — |
| 27 | `docs/devlog/devlog_26090919_qwen38fn_p0_build_delivery.md` | devlog | 26090919 | 0 | 399 | — | — |
| 28 | `docs/plan/plan_26090919_qwen38fn_p0_build_delivery.md` | plan | 26090919 | 0 | 397 | — | — |
| 29 | `docs/testlog/testlog_26090919_qwen38fn_p0_build_delivery.md` | testlog | 26090919 | 0 | 400 | — | — |
| 30 | `docs/devlog/devlog_26092300_hint_publisher_G1G3_실행서사.md` | devlog | 26092300 | 0 | 3841 | — | — |
| 31 | `docs/testlog/testlog_26092300_hint_publisher_G1G3_통합판정.md` | testlog | 26092300 | 0 | 4311 | — | — |
| 32 | `docs/devlog/devlog_26092317_native_N1_구현_라이브_서사.md` | devlog | 26092317 | 0 | 4651 | — | — |
| 33 | `docs/testlog/testlog_26102418_4모델_싱글멀티_연속캠페인_결과요약_종결.md` | testlog | 26102418 | 1 | 3124 | 0.31.0 · 1m · nv4 | — |
| 34 | `docs/testlog/testlog_26102420_PLE_mmap_0.31.0_재이식_S1_선판정_런타임패치_슬롯_정정.md` | testlog | 26102420 | 1 | 7547 | 0.31.0 · easy-vllm:0.31.0-cu133-aarch64-source · q38fn31 | — |
| 35 | `docs/plan/plan_26091108_캠페인재수행_선행_하네스교정.md` | plan | 26091108 | 1 | 19306 | 1m · nv4 | — |
| 36 | `docs/testlog/testlog_26091109_하네스교정_R0R10_역채점_음성대조.md` | testlog | 26091109 | 1 | 20402 | 1m · nv4 | — |
| 37 | `docs/report/perf_26091122_qwen38fn_24전략_종합리포트.md` | report | 26091122 | 1 | 16183 | 1m · nv4 | docs/report/perf_26091305_qwen38fn_res최소조건_후속캠페인.md |
| 38 | `docs/report/perf_26091314_qwen38fn_rtxpro6000x2_네이티브_14셀_종합.md` | report | 26091314 | 1 | 11959 | 1m · tp2 | docs/report/perf_26091322_qwen38fn_rtxpro6000x2_네이티브_10셀_동시성곡선.md |
| 39 | `docs/report/audit_26091323_qwen38fn_네이티브_캠페인_감사.md` | report | 26091323 | 1 | 20692 | 1m · nv4 | — |
| 40 | `docs/testlog/testlog_26091412_하네스교정_항목1_acceptlen승계_소급재판정.md` | testlog | 26091412 | 1 | 31725 | 1m · nv4 | — |
| 41 | `docs/plan/plan_26101011_벤치_knob_프리셋_자산화.md` | plan | 26101011 | 1 | 17408 | 1m · nv4 | — |
| 42 | `docs/plan/plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증.md` | plan | 26102401 | 1 | 7359 | 1m · nv4 | — |
| 43 | `docs/benchmark/bench_report_26102510_qwen3.8-flash-next-fp8_GB10_0.31.0.md` | bench_report | 26102510 | 1 | 6156 | 0.31.0 · easy-vllm:0.31.0-cu133-aarch64-source | — |
| 44 | `docs/benchmark/bench_report_26102510_qwen3.8-flash-next-nvfp4_GB10_0.31.0.md` | bench_report | 26102510 | 1 | 5801 | 0.31.0 · easy-vllm:0.31.0-cu133-aarch64-source | — |
| 45 | `docs/testlog/testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정.md` | testlog | 26090921 | 1 | 3946 | nv4 | — |
| 46 | `docs/benchmark/sweep_map_26091008_nv4-262k-mmp-levers.md` | sweep_map | 26091008 | 1 | 1262 | nv4 | — |
| 47 | `docs/benchmark/sweep_map_26091008_nv4-262k-res_l0-base.md` | sweep_map | 26091008 | 1 | 2607 | nv4 | — |
| 48 | `docs/report/perf_26091305_qwen38fn_res최소조건_후속캠페인.md` | report | 26091305 | 1 | 13615 | nv4 | — |
| 49 | `docs/testlog/testlog_26092223_hint_publisher_재구성_S1S5_오프라인재생_판정.md` | testlog | 26092223 | 1 | 9656 | nv4 | — |
| 50 | `docs/plan/plan_26092311_native_분산서빙_정문_구현_N1.md` | plan | 26092311 | 1 | 16270 | nv4 | — |
| 51 | `docs/testlog/testlog_26100109_v030_멀티_q38fn_fullbench_서브지침배달_판정.md` | testlog | 26100109 | 1 | 9123 | nv4 | — |
| 52 | `docs/testlog/testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정.md` | testlog | 26091001 | 1 | 4938 | — | — |
| 53 | `docs/benchmark/bench_report_26091011_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md` | bench_report | 26091011 | 1 | 3941 | — | — |
| 54 | `docs/benchmark/bench_report_26091223_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md` | bench_report | 26091223 | 1 | 3729 | — | — |
| 55 | `docs/benchmark/bench_report_26092310_56_16_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md` | bench_report | 26092310 | 1 | 2819 | — | — |
| 56 | `docs/benchmark/bench_report_26092317_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md` | bench_report | 26092317 | 1 | 6996 | — | — |
| 57 | `docs/benchmark/bench_report_26101013_qwen3.8-flash-next-nvfp4_GB10_0.30.0.md` | bench_report | 26101013 | 1 | 6377 | — | — |
| 58 | `docs/testlog/testlog_26102406_q38fn_nv4_싱글_사전판정_불가_멀티이관.md` | testlog | 26102406 | 2 | 2995 | 0.31.0 · 1m · easy-vllm:0.31.0-cu133-aarch64-source · nv4 | — |
| 59 | `docs/report/perf_26091121_qwen38fn_22셀재수행_캠페인완주.md` | report | 26091121 | 2 | 7967 | 1m · nv4 · tp2 | — |
| 60 | `docs/devlog/devlog_26091001_qwen38fn_phase0_자체이식3종_서사.md` | devlog | 26091001 | 2 | 3037 | 1m · nv4 | — |
| 61 | `docs/report/perf_26091009_qwen38fn_24cells_종합.md` | report | 26091009 | 2 | 6004 | 1m · nv4 | — |
| 62 | `docs/report/perf_26091114_qwen38fn_22셀재수행_7셀체크포인트.md` | report | 26091114 | 2 | 5862 | 1m · nv4 | — |
| 63 | `docs/report/perf_26091117_qwen38fn_22셀재수행_512k그룹완료체크포인트.md` | report | 26091117 | 2 | 7260 | 1m · nv4 | — |
| 64 | `docs/report/perf_26091322_qwen38fn_rtxpro6000x2_네이티브_10셀_동시성곡선.md` | report | 26091322 | 2 | 15328 | 1m · nv4 | — |
| 65 | `docs/benchmark/sweep_map_26091008_nv4-262k-res.md` | sweep_map | 26091008 | 2 | 2607 | nv4 | — |
| 66 | `docs/plan/plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인.md` | plan | 26100107 | 2 | 13097 | nv4 | — |
| 67 | `docs/benchmark/bench_report_26091006_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md` | bench_report | 26091006 | 2 | 3930 | — | — |
| 68 | `docs/benchmark/bench_report_26100109_qwen3.8-flash-next-nvfp4_GB10_0.30.0.md` | bench_report | 26100109 | 2 | 6680 | — | — |
| 69 | `docs/devlog/devlog_26100111_vllm030_bump_서브지침배달_멀티싱글_E2E_서사.md` | devlog | 26100111 | 2 | 4992 | — | — |
| 70 | `docs/testlog/testlog_26101108_hint발행체제_개편_서브자체발행_판정.md` | testlog | 26101108 | 2 | 6838 | — | — |
| 71 | `docs/benchmark/sweep_map_26091008_nv4-f8-262k-mmp-levers.md` | sweep_map | 26091008 | 3 | 1268 | nv4 | — |
| 72 | `docs/benchmark/sweep_map_26091008_nv4-bf-1m-mmp-levers.md` (모호 해소) | sweep_map | 26091008 | 1 | 1263 | 1m · nv4 | — |
| 73 | `docs/benchmark/sweep_map_26091008_nv4-bf-1m-res-levers.md` (모호 해소) | sweep_map | 26091008 | 1 | 1263 | 1m · nv4 | — |
| 74 | `docs/benchmark/sweep_map_26091008_nv4-f8-1m-mmp-levers.md` (모호 해소) | sweep_map | 26091008 | 1 | 1267 | 1m · nv4 | — |
| 75 | `docs/benchmark/sweep_map_26091008_nv4-f8-1m-res-levers.md` (모호 해소) | sweep_map | 26091008 | 1 | 1267 | 1m · nv4 | — |
| 76 | `docs/benchmark/bench_report_26092310_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md` (모호 해소) | bench_report | 26092310 | 1 | 2781 | — | — |
| 77 | `docs/testlog/testlog_26102414_q38fn_fp8_멀티_TP2_예산선판정_차단.md` (모호 해소) | testlog | 26102414 | 2 | 1758 | 0.31.0 · 1m · easy-vllm:0.31.0-cu133-aarch64-source · nv4 · tp2 | — |
| 78 | `docs/testlog/testlog_26102412_q38fn_fp8_싱글_사전판정_불가_멀티이관.md` (모호 해소) | testlog | 26102412 | 2 | 1180 | 0.31.0 · 1m · nv4 | — |
| 79 | `docs/testlog/testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정.md` (모호 해소) | testlog | 26102414 | 3 | 2655 | 0.31.0 · 1m · easy-vllm:0.31.0-cu133-aarch64-source · nv4 · q38fn31 · q38fn31-nv4-1m-tp2 · tp2 | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

**목표** — 계보 표에서 날짜가 가장 이른 plan 은 plan_26090918(2026-09-09)이다. Qwen3.8-Flash-Next 를 GB10 ×2 멀티노드 TP=2 에서 FP8 · NVFP4 두 변종으로, KV dtype · 컨텍스트(262K · 512K YaRN f2 · 1M YaRN f4) · PLE(resident · mmap) 전 축을 훑는 딥 캠페인이었다.

> [원문] plan_26090918_qwen38_flashnext_multi_딥캠페인 §1. 목표
> `Qwen3.8-Flash-Next`(아치 `Qwen4ExpForConditionalGeneration` · 125B/6B-active MoE + 51B PLE n-gram +
> 4B MTP · 네이티브 262K · 멀티모달)를 **멀티노드 TP=2**(GB10 ×2, RoCE 196.88Gbps)에서 구동하고,
> 전 축 광의탐색으로 최대 TPS·최대 context·최대 동시성 레시피를 확정한다.

이 셀(vLLM 0.31.0 · NVFP4 · 1M · PLE resident)을 직접 연 것은 그보다 늦은 plan_26102412(2026-10-24)다. 싱글 캠페인(plan_26102401)에서 기동 없이 산술로 불가 판정된 셀을 멀티 TP=2 에서 "지원 최대 context" 로 기동하는 계획이었다. 그 plan 이 이 셀에 세운 가정은 아래 한 줄이고, 0.31.0 stock 에 PLE mmap 이 없으니(패치 62 parked) PLE resident 로 시도하고 실패하면 음성 정직 기록한다는 조건을 붙였다.

> [원문] plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context §셀 (TP=2 이므로 노드 병렬 아님 · 순차)
> | 2 | q38fn-nv4-1m-tp2 | Qwen3.8-Flash-Next-NVFP4 | 123.53 GiB → 노드당 ≈62 GiB(+PLE 상주) | YaRN factor 4 → 1,048,576(serve 시점 주입 · `rope_scaling_translate.py`) |

**HW 출발 조건** — NVIDIA GB10 × 2 노드 · 노드당 GPU 1(00 §0.2 축 표) · 통합메모리(GPU 와 CPU 가 한 풀을 쓴다 — plan_26090918 §2.3 "GB10 통합메모리에서는 UVA CPU-offload가 무의미") · 노드 메모리 총량은 싱글 사전판정이 `MemTotal ≈121 GiB` 로 적었다(testlog_26102406 유효맥락). 블랙박스 원장 예산 선언의 mem_total_mib 124608 은 선언값이다(02 §2.5). 인터커넥트는 RoCE v2 다 — 측정 전 스캔한 manifest 가 `bandwidth_gbps: 218.33`(`measured:ib_write_bw-injected`)을 기록하고(plan_26090918 §1 의 "RoCE 196.88Gbps" 는 2026-09-09 의 옛 기재다) 측정 실행의 NCCL 은 `nccl:network = IB` · 장치가 `/RoCE` 로 찍혔다(01 §1.4 측정 env 표 #8 · #11 · #25). 분산 방식은 Ray TP=2(serving yaml `distributed-executor-backend: ray`). 호스트 드라이버는 양 노드 580.178.04(attestation 관측 · 00 §0.4)이고, 호스트 OS 는 이 계보에서 찾지 못했다(**미기록**). 이미지 좌표(CUDA 13.3.1 · torch 2.13.0a0 · NGC 26.07)는 00 §0.4 의 별도 층이다.

**모델 구조** — HF `nvidia/Qwen3.8-Flash-Next-NVFP4` · revision `fc694b54fb0174e0913e6adf86691ef85a4ead47` · base `Qwen/Qwen3.8-Flash-Next`(00 §0.2). 총 125B · 활성 6B MoE + 51B PLE n-gram + 4B MTP · 네이티브 262K · 멀티모달(plan_26090918 §1). 체크포인트 config(저작 시 직접 읽은 관측 — 계보 문서는 층 구성을 적지 않았다)는 48층 = `linear_attention` 36 + `full_attention` 12 · routed expert 512 · 토큰당 10 · `mtp_num_hidden_layers: 1` · `max_position_embeddings: 262144` · `rope_parameters.rope_type: default`(mrope interleaved · partial_rotary_factor 0.25) · `ple_layer_ids: [2]` 이다. 엔진은 선형 attention 층 때문에 mamba 계열 캐시 정렬을 켜고(`Mamba cache mode is set to 'align'` · lite_engine_head L80) GDN prefill 커널을 고른다(lite_engine L518). 크기는 safetensors index `metadata.total_size` 132,639,846,394 B = 123.53 GiB 이고 NAS `du` 의 248G 는 부풀려진 값이다(testlog_26102406 §근거 1 · plan_26090918 §2.1 "du 는 이 NAS에서 ~2배 과대보고") — 측정 엔진 로그도 `Checkpoint size: 123.57 GiB` 를 찍는다(lite_engine L638). 양자화는 ModelOpt MIXED_PRECISION 으로 NVFP4 층 48 · PLE FP8 1 · MTP FP8_PB_WO 1 · 제외 모듈 292개는 bfloat16 이다(00 §0.4). PLE 테이블은 파일 하나(`model-fp8-mtp-ple.safetensors` 50.03 GiB)에 128 샤드 F8_E4M3 합계 47.68 GiB 다(testlog_26102420 §2).

**stock 상태의 장벽**(자세히는 2.2)
- 0.29.0rc6 은 NVFP4 혼합 체크포인트의 PLE 를 받지 못할 것으로 예상됐다(W1 — 그 계보는 빌드 패치 60 으로 넘었고, 0.31.0 stock 은 같은 체크포인트의 PLE 를 패치 없이 FP8 방식으로 초기화했다).
- 0.29 계보의 첫 스모크들은 예산 미선언 · MoE 백엔드 명시 · cudagraph 캡처 피크에서 차례로 막혔다(W2 · W3 · W4 · W5).
- 0.29 계보에서 NVFP4 + PLE resident + 1M 은 로드 중 호스트 memwatch 에 사살됐다(W6).
- 0.31.0 은 YaRN 계열의 최대 길이 의미가 바뀌어 0.29 번역기 산출만으로는 1M 이 거부된다(W8).
- 0.31.0 stock 에는 PLE mmap 경로가 없다(R2) · 싱글 노드는 체크포인트가 들어가지 않는다(R1).

**선례와 사람의 결정** — 첫 plan 이 모은 외부 선례는 셋이다(plan_26090918 §2.3): dolf3131 (싱글 NVFP4 · `VLLM_PLE_CPU_OFFLOAD=1`+스왑 · 디코드 33.0 t/s · MTP k=3), blazux (싱글 · PLE NVMe mmap · 디코드 ~37 t/s · YaRN f2→500K 검증 · "1M은 fp8 KV 전제" · 멀티노드 미구현 명시), recipes.vllm.ai 공식 레시피. 뒤의 res 후속 캠페인은 NVIDIA 포럼의 같은 체크포인트 수치를 E 로 골랐다 — 1 Spark 32.5 median(PLE mmap) · 2 Spark SPEED 53.7 median(PLE resident · MTP3 · KV fp8)(testlog_26091304 §2). 사람은 첫 plan 에서 FP8 · NVFP4 양 변종 전부 · 자체이식 최대한도 · YaRN 유/무 · 시간비용 무제를 골랐고(plan_26090918 머리 사용자 결정), 이 셀의 직접 계획에는 "승인: 제안 순서대로 (Recommended)" 로 멀티 3셀 기동을 승인했다(plan_26102412 §Execution approval). 이 셀은 PLE mmap 재이식(plan_26102420) **이전**의 stock 형상이고, 재이식된 mmap 플러그인은 이 셀에 실리지 않는다(01 §1.1 `runtime_patch` 안 실림) — mmap 동치 시험 셀 `q38fn31-nv4-mmap-1m-tp2` 는 다른 셀이다.

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계 미관측** — 재생 모드(캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음)) — 캠페인 선언을 관측하지 못한다 · 경계 미관측 — 라벨(셀 이름)로 묶은 행 전부를 이 셀의 행으로 센다(같은 셀 이름의 앞선 캠페인 행이 섞였을 수 있다)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-25T03:13:55Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) · sub = 2026-06-13T22:25:01Z → 2026-10-24T00:02:35Z(`sync_staging/sub_docs/logs/sub/events/2026-06.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-07.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-08.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-09.jsonl` · `sync_staging/sub_docs/logs/sub/events/2026-10.jsonl` · `sync_staging/sub_docs/logs/sub/events/thermal.jsonl` · `sync_staging/sub_docs/logs/sub/events/watchdog.jsonl` · 회수 미러가 가린 옛 로컬 사본 `docs/logs/sub/events/2026-09.jsonl`) — ⚠ sub(마지막 행 2026-10-24T00:02:35Z < 측정 끝 2026-10-25T02:27:13Z): 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-q38fn31-nv4-1m-tp2` | 2026-10-24T05:01:27Z | 없음 | 없음 | `budget_clear` 2026-10-24T05:03:25Z | 2 |
| 2 | main | `smoke-q38fn31-nv4-1m-tp2` | 2026-10-24T05:03:48Z | 1회 · 첫 2026-10-24T05:27:36Z | 없음 | `budget_clear` 2026-10-24T05:52:51Z | 3 |
| 3 | main | `smoke-q38fn31-nv4-1m-tp2` | 2026-10-24T12:22:03Z | 1회 · 첫 2026-10-24T12:46:06Z | 없음 | `budget_clear` 2026-10-24T12:47:59Z | 3 |
| 4 | main | `smoke-q38fn31-nv4-1m-tp2` | 2026-10-24T20:16:04Z | 1회 · 첫 2026-10-24T20:39:46Z | 없음 | `budget_clear` 2026-10-24T20:50:53Z | 3 |
| 5 | main | `smoke-q38fn31-nv4-1m-tp2` | 2026-10-25T01:52:06Z | 4회 · 첫 2026-10-25T02:16:29Z | 없음 | `budget_clear` 2026-10-25T02:27:37Z | 11 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

**기동 시도와 벽의 대응** — 원장은 이 셀 라벨(`smoke-q38fn31-nv4-1m-tp2`)로 기동 시도 5번을 묶는다(사살 · 트립 · 거부 칸은 모두 `없음` — 메인 원장 기준이다. 서브 원장은 2026-09-11 뒤 행이 회수되지 않아 서브 쪽 사살은 이 표에 나올 수 없다). 문서 · 로그로 대응이 확인되는 것은 셋이다: 시도 1(2026-10-24T05:01:27Z 창 · 선언 후 1분 58초에 닫힘)은 `serve_fail_q38fn31-nv4-1m-tp2/master_exited.log` 의 컨테이너 시각 `10-24 05:02:01` 기동 인자 줄과 그 끝의 `ValidationError` 를 담는다(W8) · 시도 2(05:03:48Z ~ 05:52:51Z)는 캠페인 lite raw(`output/multi/benchlog/lite_raw_q38fn31-nv4-1m-tp2.json` · measured_utc 2026-10-24T05:51:18Z)를 창 안에 품으므로 testlog_26102414 의 3차(health 200 · 1M needle · lite 25.91 t/s)로 읽힌다 · 시도 5(2026-10-25T01:52:06Z 창)가 이 페이로드의 측정 기동이다(선언 → API 서버 기동 02:15:29Z · 23분 23초). 시도 4(20:16:04Z ~ 20:50:53Z)는 닫힘 직전 20:50:28Z 에 생성된 무효 리포트 `bench_report_26102505_qwen3.8-flash-next-nvfp4_GB10_0.31.0`(첫 full 패스 · 도구 vLLM bench · R6)과 시각이 겹친다 — **시각 대조에 의한 추론**이다(그 리포트는 계보 표 밖). 시도 3(12:22:03Z ~ 12:47:59Z)이 무엇이었는지는 계보 문서에 대응 기록이 없어 **미확인**이다(02 Q6). testlog_26102414 의 1차 시도(`서브 통로 미완결` · W7)는 원장에 선언 행이 없다 — 선언 전에 멈춘 것으로 보인다(추론).

아래 W1 ~ W6 은 0.29.0rc6 계보(다른 셀 · 다른 vLLM)의 벽이고, W7 · W8 이 이 셀(0.31.0)에서 넘은 벽이다. 앞의 것들은 이 셀의 노브가 왜 지금 값인지(eager · MoE 백엔드 미명시 · KV 클램프 · 예산 선언)를 설명하는 계보로 싣는다.

### W1 (0.29 계보) NVFP4 혼합 체크포인트의 PLE 를 stock 이 받지 않는다

첫 plan 의 정찰은 0.29.0rc6 이미지의 PLE 양자화 방식 선택 함수가 FP8 전용이라 NVFP4(ModelOpt 혼합) 체크포인트의 PLE 가 거부될 것으로 봤다. 빌드 패치 60(MIXED_PRECISION PLE 라우트 · MTP FP8_PB_WO MoE 디스패치 · MTP 층 리맵 3 hunk)으로 넘었고 스모크 5차에서 health 200 + 추론으로 판정했다(testlog_26090921 §판정). 0.31.0 은 이 자리를 stock 이 처리한다 — 이 셀의 이미지에는 빌드 패치가 하나도 없고(01 §1.1 원장 `context_inventory` = `.gitkeep` 뿐), 측정 기동 로그에서 PLE 는 `quantization_method=Qwen4ExpPLEFp8EmbeddingMethod` 로 초기화됐다(lite_engine L631 · L649).

```hint-event
id: W1
kind: wall
증상: 0.29.0rc6 stock 에서 NVFP4 혼합 체크포인트의 PLE 양자화 방식이 None 이 되어 로드 거부가 예상됨
서명: "동 함수가 FP8 전용 — `modelopt_fp4` 시 None (커뮤니티 동일 장애)"
원인: 0.29.0rc6 의 PLE 양자화 방식 선택 함수가 Fp8Config 만 받았다
해소: 0.29 계보는 빌드 패치 60(PLE mixed 라우트 + MTP 디스패치 + 리맵) · 0.31.0 은 stock 이 처리(이 셀 패치 0)
검증: testlog_26090921 §판정 스모크 5차 PASS · 이 셀 측정 기동 lite_engine L631 Initialized PLE embedding ... Qwen4ExpPLEFp8EmbeddingMethod
전이등급: judgment
출처: [plan_26090918_qwen38_flashnext_multi_딥캠페인 §2.2 우리 이미지, testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정, output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log]
```

### W2 (0.29 계보) 예산 미선언 · KV 클램프 부재로 스모크 진입 차단

P0-1 스모크 1차는 호스트 메모리 예산 선언과 KV 절대 클램프가 없어 게이트가 막았다. 정상 차단이며 우회하지 않고 `kv-cache-memory-bytes` 클램프와 예산 선언을 갖춰 진입했다. 이 셀도 같은 절차로 선언 뒤 기동했다(01 §1.4 원장 표 · 시도마다 `budget_declare` → `budget_honored`).

```hint-event
id: W2
kind: wall
증상: P0-1 스모크 1차가 기동 전에 차단됨
서명: "예산 미선언 + KV 클램프 부재 — 게이트가 제 일을 함"
원인: 호스트 메모리 예산 선언과 KV 절대 클램프 없이 기동하려 했다(정상 차단)
해소: kv-cache-memory-bytes 절대 클램프 + 예산 선언(budget_declare) 후 진입
검증: testlog_26090921 §판정 스모크 5차 PASS · 이 셀 원장 docs/logs/main/events/2026-10.jsonl 643행 budget_declare 와 watchdog.jsonl budget_honored
전이등급: arch-invariant
출처: [testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정, docs/logs/main/events/2026-10.jsonl, docs/logs/main/events/watchdog.jsonl]
```

### W3 (0.29 계보) `moe-backend: triton` 명시 — NVFP4 MoE 미지원

스모크 2차는 `moe-backend: triton` 으로 띄웠다가 엔진이 NVFP4 MoE 를 지원하지 않는다며 fail-loud 로 멈췄다(첫 plan §2.3 이 공식 레시피의 FP8 예시에 triton 이 있다고 적지만 스모크 값을 거기서 옮겼다는 기록은 없다). 처방은 백엔드를 명시하지 않는 것(auto)이다. 이 셀의 yaml 에도 `moe-backend` 가 없고, 측정 기동에서 엔진은 `FLASHINFER_CUTLASS` 를 골랐다(lite_engine L520 · L648).

```hint-event
id: W3
kind: wall
증상: P0-1 스모크 2차가 MoE 백엔드 검사에서 fail-loud 로 종료
서명: "`moe-backend: triton` — NVFP4 MoE 미지원(엔진 fail-loud)"
원인: 명시한 triton MoE 백엔드가 NVFP4 MoE 를 지원하지 않는다
해소: moe-backend 를 명시하지 않음(auto) — 이 셀의 엔진은 FLASHINFER_CUTLASS 를 자동 선택
검증: testlog_26090921 §판정 최종 설정 moe-backend 미설정(auto) · 이 셀 lite_engine L520 Using 'FLASHINFER_CUTLASS' NvFp4 MoE backend
전이등급: arch-scaled
출처: [testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정, output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log]
```

### W4 (0.29 계보) `moe-backend: cutlass` 명시가 패치 가드와 충돌

스모크 3차는 cutlass 를 명시했고, 패치 60 의 B1 경로(MTP FP8_PB_WO MoE 디스패치)가 걸어 둔 `allow_vllm_cutlass=False` 가드와 충돌했다. 이 벽은 0.29 계보의 빌드 패치가 만든 가드 충돌이라 패치가 없는 이 셀에 같은 가드가 있는지는 확인하지 않았다 — 이 셀은 MoE 백엔드를 명시하지 않았으므로 걸릴 자리가 없었다.

```hint-event
id: W4
kind: wall
증상: P0-1 스모크 3차가 MoE 백엔드 가드 충돌로 종료
서명: "`moe-backend: cutlass` 명시가 B1 경로 `allow_vllm_cutlass=False` 가드에 충돌"
원인: 패치 60 의 B1 경로가 vllm cutlass 를 금지하는 가드를 두었는데 백엔드를 cutlass 로 명시했다
해소: moe-backend 를 명시하지 않음(auto)
검증: testlog_26090921 §판정 스모크 5차 PASS(최종 설정 moe-backend 미설정)
전이등급: judgment
출처: [testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정]
```

### W5 (0.29 계보) cudagraph 캡처 구간의 메모리 피크로 워치독 사살

스모크 4차는 그래프 캡처 구간에서 MemAvailable 이 사살 문턱 아래로 내려가 워치독이 컨테이너를 죽였다. 5차에서 `enforce-eager: true` 로 캡처를 없애 통과했고, 그 testlog 는 eager 를 "스파이크 분리" 로 적었다. ⚠ 그때 조건은 이 셀과 다르다 — MTP k=3 · gmu 0.85 · KV 20 GiB(같은 testlog §판정 최종 설정)이다. 이 셀(MTP 없음 · gmu 0.80 · KV 14 GiB · max-num-seqs 1)에서 cudagraph 를 다시 시도한 기록은 없다(02 Q3 · 01 V7 은 승계값).

> [원문] testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정
> | 스모크 4차 | **워치독 TRIP**(방어 성공) | 그래프 캡처 구간 MemAvailable 9,378MiB < 10,240MiB → docker kill |
> | 스모크 5차(eager) | **PASS** | health 200 · 추론 content_len=117 fr=stop · SMOKE_RC=0 |

```hint-event
id: W5
kind: wall
증상: P0-1 스모크 4차 중 cudagraph 캡처 구간에서 컨테이너가 워치독에 사살됨
서명: "그래프 캡처 구간 MemAvailable 9,378MiB < 10,240MiB → docker kill"
원인: 캡처 구간의 일시 메모리 피크가 호스트 MemAvailable 사살 문턱(10,240MiB)을 넘었다(MTP k=3 · gmu 0.85 · KV 20 GiB 조건)
해소: enforce-eager true 로 cudagraph 캡처 제거
검증: testlog_26090921 §판정 스모크 5차(eager) PASS
전이등급: arch-scaled
출처: [testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정]
```

### W6 (0.29 계보) NVFP4 + PLE resident + 1M 이 로드 중 memwatch 에 사살

22셀 재수행 캠페인의 `nv4-bf-1m-res` · `nv4-f8-1m-res` 는 둘 다 서브 노드에서 systemd memwatch 절대층(MemAvailable < 10,240MiB)에 사살됐다. 같은 1M 의 mmap 셀은 4/4 서빙됐다(testlog_26091121 §판정 결과). 뒤의 res 후속 캠페인이 262k 셀에서 관측한 것은 KV 를 줄여도 로드 피크가 그 몫을 되먹었고 gmu 0.85 → 0.80 이 피크 여유를 열었다는 것이다(testlog_26091304 §3.2) — 그 기전(gmu 가 총량 cap 으로 작용)은 2026-09-14 정정으로 가설(강등)이다(02 §2.5). 이 셀은 KV 를 fp8 14 GiB 로 두고 gmu 0.80 · max-num-seqs 1 · max-num-batched-tokens 2048 로 기동해 TRIP 0 이었다(testlog_26102414 §1 선례 대비). 0.29 1M res 셀들의 트리플렛(`output/multi/configs/nv4-{bf,f8}-1m-res.yaml` · 계보 표 밖)은 gmu 0.85 · KV 21474836480 · MTP k=3 · max-num-seqs · max-num-batched-tokens 미설정이다 — 이 셀과 여러 노브가 함께 다르므로 버틴 이유를 KV 축소 하나로 적지 않는다(설정 변경 전부는 02 §2.5).

> [원문] testlog_26091121_qwen38fn_22셀재수행_캠페인완주_판정 §비-measured 셀(같은 체크포인트) — 4건
> | `nv4-f8-1m-res`(#18) | `serve_failed` / `watchdog_kill_ack` | 동상, 11:10:34Z TRIP MemAvailable=9945MiB<10240MiB → kill_ack 11:10:38Z |

```hint-event
id: W6
kind: wall
증상: 0.29.0rc6 의 NVFP4 · PLE resident · 1M 셀 2개가 로드 중 서브 노드에서 사살됨
서명: "TRIP MemAvailable=9785MiB<10240MiB(source=unit:easy-vllm-memwatch)"
원인: 로드 피크에서 서브 MemAvailable 이 memwatch 절대 문턱 아래로 내려갔다(gmu 0.85 · KV 20 GiB · MTP k=3 조건 · 피크를 정하는 기전은 가설(강등))
해소: 이 셀은 KV fp8 14 GiB · gmu 0.80 · max-num-seqs 1 · max-num-batched-tokens 2048 로 재확정(설정 차이 전부는 02 §2.5)
검증: testlog_26102414 §1 선례 대비 TRIP 0 · 이 셀 원장 시도 5 사살 칸 없음(메인 원장 기준)
전이등급: arch-scaled
출처: [testlog_26091121_qwen38fn_22셀재수행_캠페인완주_판정 §비-measured 셀(같은 체크포인트) — 4건, testlog_26091304_qwen38fn_res최소조건_262k재발행_판정 §3.2, testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §1. 기동과 교정]
```

### W7 서브에 compose 가 없어 멀티 스모크가 진입하지 못함

멀티 브랜치에서 처음 이 셀을 띄우려 했을 때 서브 노드에 `output/multi/docker-compose.yaml` 이 없어 스모크가 서브 통로 미완결로 멈췄다. 배달 평면 절차(`sync_to_sub.sh --branch multi` · dry-run → apply · 승인 앵커 plan_26102412)로 해소했다. 원장에 이 시도의 선언 행이 없다(위 대응 문단).

```hint-event
id: W7
kind: wall
증상: 멀티 스모크가 서브 통로 미완결로 기동 전에 멈춤
서명: "`FAIL 서브 통로 미완결`(서브에 `output/multi/docker-compose.yaml` 부재)"
원인: 서브 노드에 멀티 브랜치 산출물(compose)이 배달되지 않았다
해소: sync_to_sub.sh --branch multi(dry-run 후 apply · 승인 앵커 plan_26102412)
검증: testlog_26102414 §1 3차 health 200 · 스모크 rc=0
전이등급: judgment
출처: [testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §1. 기동과 교정]
```

### W8 0.31.0 은 YaRN 최대 길이를 factor 로 다시 늘리지 않는다

0.29 계보의 YaRN 번역기(`rope_scaling_translate.py` · 하네스 교정 R8)가 낸 hf-overrides 는 `rope_parameters` 만 담았고, 0.29.0rc6 에서는 그것으로 1M 이 서빙됐다(testlog_26091121 §R8). 0.31.0 은 기동 인자 단계에서 1M 을 거부했다. 원인 판정은 testlog 의 소스 독해다 — `config/model.py:_get_and_verify_max_len` 이 YaRN 계열에서 `max_position_embeddings` 가 이미 확장값이라고 가정해 factor 를 재적용하지 않으므로 derived 가 262,144 로 남는다. 처방은 hf-overrides 에 `text_config.max_position_embeddings=1048576` 을 함께 싣는 것이다. 우회 env `VLLM_ALLOW_LONG_MAX_MODEL_LEN` 은 YaRN 미적용 위험 때문에 쓰지 않았다(R4). 통과는 측정 기동 `non-default args` 의 hf_overrides 에 `max_position_embeddings: 1048576` 이 실리고 `Using max model len 1048576`(lite_engine_head L62 · L72)이 찍힌 것으로 판정한다. YaRN 이 실제로 위치 보간에 쓰였는지는 needle 이 경험적으로만 말한다(02 Q4).

> [원문] output/multi/benchlog/serve_fail_q38fn31-nv4-1m-tp2/master_exited.log §L116-117
> pydantic_core._pydantic_core.ValidationError: 1 validation error for ModelConfig
>   Value error, User-specified max_model_len (1048576) is greater than the derived max_model_len (max_position_embeddings=262144.0 or model_max_length=None in model's config.json). To allow overriding this maximum, set the env var VLLM_ALLOW_LONG_MAX_MODEL_LEN=1. VLLM_ALLOW_LONG_MAX_MODEL_LEN must be used with extreme caution. If the model uses relative position encoding (RoPE), positions exceeding derived_max_model_len lead to nan. If the model uses absolute position encoding, positions exceeding derived_max_model_len will cause a CUDA array out-of-bounds error. [type=value_error, input_value=ArgsKwargs((), {'model': ...nderer_num_workers': 1}), input_type=ArgsKwargs]

```hint-event
id: W8
kind: wall
증상: 기동 인자 검증에서 max_model_len 1048576 이 derived 262144 보다 크다며 ValidationError 로 종료
서명: "User-specified max_model_len (1048576) is greater than the derived max_model_len (max_position_embeddings=262144.0"
원인: 0.31.0 의 _get_and_verify_max_len 은 YaRN 계열에서 max_position_embeddings 를 이미 확장된 값으로 보고 factor 를 재적용하지 않는다(testlog 의 소스 독해)
해소: hf-overrides text_config 에 max_position_embeddings 1048576 을 rope_parameters(YaRN factor 4 · original 262144)와 함께 실음
검증: 측정 기동 lite_engine_head L62 non-default args 의 max_position_embeddings 1048576 · L72 Using max model len 1048576 · testlog_26102414 §2 needle 1,001,130 PASS
전이등급: arch-invariant
출처: [output/multi/benchlog/serve_fail_q38fn31-nv4-1m-tp2/master_exited.log, testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §1. 기동과 교정, output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log]
```

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: 싱글 노드 TP=1 로 NVFP4 1M 기동(셀 q38fn-nv4-1m · vLLM 0.31.0)
기각사유: 체크포인트 123.53 GiB 가 단일 노드 예산 121.7 GiB 를 가중치만으로 넘고 recipe.py estimate 18개 후보가 전부 FAIL — 기동 시도 없이 산술로 사전 불가 판정 후 멀티로 이관(시도 0 · 기동 실패가 아니다)
재개조건: 상주 바이트를 줄이는 PLE mmap 이 있을 때(뒤에 런타임 플러그인 셀 q38fn31-nv4-mmap-1m-tp1 이 그 경로로 열렸다 · 다른 셀)
출처: [testlog_26102406_q38fn_nv4_싱글_사전판정_불가_멀티이관 §근거 (전부 관측·산출물 인용), testlog_26102418_4모델_싱글멀티_연속캠페인_결과요약_종결 §결과표]
```

```hint-event
id: R2
kind: rejected
시도: 0.29 계보처럼 PLE 를 NVMe mmap 으로 내려 상주 바이트를 줄이기(빌드 패치 62)
기각사유: 0.31.0 stock 에 VLLM_PLE_MMAP 경로가 없고 패치 62 는 0.30.0 에서 anchor 소실로 parked 됐다 — 이 셀은 PLE resident(engram cpu_offload false)로 시도했다
재개조건: PLE mmap 을 0.31 에 다시 넣었을 때(뒤에 plan_26102420 이 런타임 플러그인으로 재저작했다 — 이 셀에는 실리지 않았다)
출처: [testlog_26102406_q38fn_nv4_싱글_사전판정_불가_멀티이관 §근거 (전부 관측·산출물 인용), plan_26102412_4모델_멀티캠페인_Solar_Qwen_TP2_최대context §셀 (TP=2 이므로 노드 병렬 아님 · 순차)]
```

```hint-event
id: R3
kind: rejected
시도: PLE 를 CPU 로 내리는 stock 경로(VLLM_PLE_CPU_OFFLOAD · Engram cpu_offload 의 PinnedHost 백엔드)
기각사유: PinnedHost 는 테이블 전체를 pinned 메모리로 재료화하는데 GB10 은 호스트와 GPU 가 한 풀이라 절감이 0 이다 — 이 셀은 engram-config cpu_offload false 로 두었다(켜서 잰 기록은 없다 · 판단 근거는 메모리 구조)
출처: [testlog_26102420_PLE_mmap_0.31.0_재이식_S1_선판정_런타임패치_슬롯_정정 §3. stock 0.31.0 의 두 백엔드가 GB10 에서 왜 상주를 못 줄이나, testlog_26102406_q38fn_nv4_싱글_사전판정_불가_멀티이관 §근거 (전부 관측·산출물 인용)]
```

```hint-event
id: R4
kind: rejected
시도: W8 을 env VLLM_ALLOW_LONG_MAX_MODEL_LEN 으로 우회
기각사유: 길이 검사만 끄고 YaRN 이 적용되지 않을 위험이 있어 쓰지 않았다 — hf-overrides 에 max_position_embeddings 를 함께 싣는 쪽을 골랐다(우회 경로로 기동한 기록은 없다)
출처: [testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §1. 기동과 교정]
```

```hint-event
id: R5
kind: rejected
시도: (0.29 계보 · 262k res 셀) PLE resident 사살을 KV 클램프를 더 내려서(20,480 → 8,192 MiB/노드) 피하기
기각사유: KV 에서 회수한 12,288 MiB 를 로드 피크가 12,392 MiB 되먹어 여전히 사살됐고 gmu 0.85 → 0.80 이 여유를 열었다(262k 셀의 관측 · gmu 가 천장으로 작용한다는 기전은 가설(강등) · 1M 에서 같은 대조는 없다)
출처: [testlog_26091304_qwen38fn_res최소조건_262k재발행_판정 §3.2]
```

```hint-event
id: R6
kind: rejected
시도: sweep_bench.sh 를 --tool 없이 돌린 첫 full 패스(기본 도구 vLLM bench · 이 셀 판정점 26.92 t/s)
기각사유: full 정의(GuideLLM × 반복 ≥3)를 충족하지 않는 도구로 측정돼 무효 — 구 리포트 26102505_* · 26102506_* 폐기 · --tool guidellm 으로 재측정한 것이 03 의 수치다
출처: [testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2. 하네스 결함 · 처방, devlog_26102504_PLE_mmap_재이식_full벤치_hint발행_서사 §시도 · 폐기]
```

**요약** — 이 셀의 형상을 좁힌 것은 세 갈래다. ① 노드 수: 싱글은 산술로 배제돼(R1) 처음부터 TP=2 다. ② PLE: 0.31.0 stock 에는 mmap 이 없고(R2) CPU offload 는 통합메모리에서 아무것도 거두지 못하므로(R3) resident 외 선택지가 없었다. 그래서 상주 바이트를 줄이는 대신 KV · gmu · 배치로 로드 피크를 다뤘다(R5 의 262k 관측 · 기전은 가설(강등) · W6). ③ 길이 · 측정: 1M 은 길이 검사 우회가 아니라 오버라이드로 열었고(R4 · W8), 측정은 GuideLLM 재측정분만 유효하다(R6). MoE 백엔드를 명시하지 않는 것과 eager 는 0.29 계보의 벽(W3 · W4 · W5)에서 온 승계다.

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

### M1 PLE resident 축은 실패하는 축이라는 판정

0.29 의 24전략 종합 리포트는 하네스 교정 뒤 resident 셀 5건이 같은 floor 위에서 전부 사살된 것을 근거로 resident 축을 "12/12 실패" 로 취급하라고 결론 냈다. 이틀 뒤 res 후속 캠페인(camp-26091216)이 같은 리포트 안에 정정을 달았다 — 그 floor 를 만든 KV 20,480 MiB 가 262k 이론 최소치의 6.7배 과잉이었고 진짜 제약은 gmu 천장이었다고 적었으며(그 기전 서술은 뒤의 2026-09-14 정정으로 관측만 남고 가설로 내려갔다), KV 8,192 MiB · gmu 0.80 에서 resident 262k 셀이 46.70 t/s 로 서빙돼 mmap 자매 셀을 앞섰다. 이 셀이 0.31.0 에서 resident 로 1M 을 시도한 근거가 이 정정이다(testlog_26102406 §처분 "res 축 선례 camp-26091216 46.7 t/s"). 다만 그 정정은 262k 셀의 기전이다 — 1M resident 가 같은 기전으로 버티는지는 이 셀의 기동 성공이 결과로 보일 뿐 대조 실험은 없다.

> [원문] perf_26091122_qwen38fn_24전략_종합리포트 §4. 정직성 노트 — nv4-bf-262k-res 레거시 "성공"은 재현 대상이 아니다
> **결론(발행 시점)**: 24전략 중 **실질적으로 신뢰 가능한 권고 축은 mmp 12/12**이며, res 축은
> (레거시 1건의 불확실한 예외를 제외하면) **12/12 실패**로 취급하는 것이 안전하다.

```hint-event
id: M1
kind: misdiagnosis
증상: 하네스 교정 뒤 NVFP4 · PLE resident 셀 5건이 동일 floor 위에서 전부 워치독에 사살됨
서명: "(레거시 1건의 불확실한 예외를 제외하면) **12/12 실패**로 취급하는 것이 안전하다"
원인: 사살을 축(resident) 자체의 한계로 읽었다 — floor 를 만든 KV 과잉과 gmu 천장을 가르지 않았다
해소: resident 는 예산 배분 문제였다 — KV 8,192MiB · gmu 0.80 에서 262k resident 가 46.70 t/s 로 서빙(mmap 자매 셀 38.81 을 전 동시성에서 앞섬)
검증: testlog_26091304 §3.2(KV 회수가 피크에 먹힘 · gmu 0.85 → 0.80 이 5,562 MiB 를 엶) · perf_26091122 §4 정정 박스
전이등급: judgment
현재지위: 반증
출처: [perf_26091122_qwen38fn_24전략_종합리포트 §4. 정직성 노트 — nv4-bf-262k-res 레거시 "성공"은 재현 대상이 아니다, testlog_26091304_qwen38fn_res최소조건_262k재발행_판정 §3.2]
```

### M2 thinking on 으로 잰 needle 첫 시도의 FAIL

1M 검증 needle 의 첫 시도는 thinking on · max_tokens 32 로 돌았고 FAIL 이 났다. testlog 는 이것을 응답이 추론 서두에서 잘린 측정 결함으로 판정하고 thinking off 로 재시도해 전 단계 PASS 를 얻었다 — 검색 실패가 아니었다. 같은 needle 을 다시 돌리는 수신자는 thinking 을 끄거나 max_tokens 를 충분히 줘야 같은 오판을 피한다.

```hint-event
id: M2
kind: misdiagnosis
증상: 1M 셀 needle 첫 시도가 FAIL
서명: "thinking on · max_tokens 32 의 첫 시도 FAIL 은 추론 서두에서 잘린 측정 결함이며 검색 실패가 아님"
원인: 출력 예산 32 토큰이 추론 서두에서 소진돼 답이 나오기 전에 잘렸다
해소: thinking off · temperature 0 으로 재시도 — 102,930 ~ 1,001,130 토큰 전 단계 PASS
검증: testlog_26102414 §2 needle 표
전이등급: arch-invariant
현재지위: 반증
출처: [testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §2. needle(MAGNOLIA-7734 · 증분 프리필 · thinking off · temperature 0)]
```

### M3 yaml 의 `async-scheduling: false` 가 비동기 스케줄링을 끈다는 기재

이 셀의 yaml 에는 `async-scheduling: false` 가 있다. 그러나 측정 기동의 엔진 `non-default args`(lite_engine_head L62)에 `async_scheduling` 키가 없다 — 설정 파일의 false 가 엔진 인자로 넘어가지 않았다. 계보 밖 문서 testlog_26102500 §4 는 vLLM 설정 파일 파서가 불리언 false 를 버려 비동기 스케줄링이 기본값(True)으로 켜지며, 그 이전 셀 yaml 전부에 같은 줄이 있었으므로 이전 측정은 async 가 켜진 상태였다고 적는다(구성 기재만 정정 · 결과 유효성은 그대로). 이 셀의 엔진 로그에는 resolve 값을 찍는 줄이 없지만(로그 grep 0건) 실효값은 이미지 안 소스 규칙으로 정해진다 — `vllm/config/vllm.py` L1576 ~ L1635 는 async_scheduling 이 None 이면 pooling · 비-EAGLE 계 spec · 실행기 미지원 등이 아닐 때 켜고, 실행기는 `VLLM_USE_RAY_V2_EXECUTOR_BACKEND` 기본값 1(`vllm/envs.py`)로 RayExecutorV2 이며(로그의 `RayWorkerProc` 액터) 그 상위 MultiprocExecutor 의 `supports_async_scheduling()` 은 True 다(`multiproc_executor.py` L559 ~ L561). 이 셀은 spec 없음 · PP=1 · generate 이고 비활성 경로의 경고(`Async scheduling will be disabled …`)도 로그에 없다 — 따라서 이 측정은 **비동기 스케줄링 켜짐**이었다(이미지 파일 + 로그로 판정 · 이미지는 docker create · cp 로 읽었다). 이 셀은 동기 스케줄링을 요구하는 구성 요소(V2 logits processor 등)가 없어 그 차이로 기동이 막히지는 않았다.

```hint-event
id: M3
kind: misdiagnosis
증상: yaml 에 async-scheduling false 를 두었으나 엔진 non-default args 에 그 키가 없다
서명: "async-scheduling: false"
원인: vLLM 설정 파일 파서가 불리언 false 를 non-default 인자로 넘기지 않는다(계보 밖 testlog_26102500 §4 의 독해)
해소: 동기 스케줄링이 필요하면 러너 명령줄에 --no-async-scheduling 을 두고 non-default args 에서 확인한다 · 이 셀의 실효값은 엔진 기본 규칙으로 켜짐이다(V8)
검증: 측정 기동 lite_engine_head L62 non-default args 에 async_scheduling 없음
전이등급: arch-invariant
현재지위: 반증
출처: [q38fn31-nv4-1m-tp2.yaml, output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log]
```

### M4 full 벤치 판정 testlog 의 bench 예산 기재

full 벤치 판정 testlog 는 유효맥락에 전 셀의 측정 구성을 `bench 예산 2048 MiB` 로 적고, §2 표 2번 행에 4096 MiB 가 "arm_ceiling 한도 초과" 로 BLOCK 됐다고 적었다. 이 셀의 스윕 색인은 `bench_knobs.knobs.bench_budget_mib` 를 `4096`(reason `host_budget`)으로 기록한다(sweep_index.json L346-352). 이 셀의 선언값으로 `budget_preflight.py --mem-total-mib 124608 --weights-mib 63266 --kv-mib 14336 --overhead-mib 24800 --bench-budget-mib 4096` 을 돌리면 `bench_ok: true` · `bench_budget_max_mib: 11966` 이 나온다 — 벤치 진입 게이트는 `floor − bench_budget ≥ abs_band(10240)` 이고 이 셀 floor 는 22206 MiB 다. BLOCK 이 난 것은 testlog 가 괄호로 적은 다른 셀들(Qwen TP1 · Solar multi)이며, 그쪽의 원인도 arm_ceiling 대비가 아니라 같은 게이트다. GuideLLM 컨테이너의 `--memory` 실제 값은 로그가 echo 하지 않아 관측하지 못했다 — 기록값은 4096 이다.

```hint-event
id: M4
kind: misdiagnosis
증상: 판정 testlog 가 전 셀의 bench 예산을 2048 MiB 로 기재
서명: "판정점 동시성 1 · 합성 데이터셋 · bench 예산 2048 MiB"
원인: 일부 셀(Qwen TP1 · Solar multi)의 BLOCK 처방을 전 셀 측정 구성으로 일반화해 적었다 · BLOCK 원인도 arm_ceiling 대비로 잘못 적었다
해소: 이 셀 스윕 색인의 bench_budget_mib 기록은 4096 · 이 셀 floor 22206 에서 벤치 예산 상한은 11966 MiB(budget_preflight.py 계산 · 게이트 floor − bench ≥ abs_band 10240)
검증: output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/sweep_index.json L346-352 · budget_preflight.py 출력 bench_ok true
전이등급: judgment
현재지위: 반증
출처: [testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2. 하네스 결함 · 처방, output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/sweep_index.json]
```

**교정 묶음**
- testlog_26091109 하네스 교정 R0~R10(11개 검토) — 0.29 캠페인을 위한 교정이라 이 셀 이전에 끝났고 이 셀의 수치를 전 → 후로 바꾸지 않았다. 이 셀의 **입력을 정한** 항목만 적는다:
  - R2 — 예산 게이트 산술이 resident/mmap 을 가른다(resident 는 PLE 를 빼지 않는다) — 이 셀 선언의 weights_mib 63266 · floor 22206 이 그 산술의 산출이다(원장 643행).
  - R3 — GuideLLM 컨테이너 몫이 예산에 이어졌다 — M4 의 벤치 진입 게이트가 그것이다.
  - R8 — YaRN 번역기(`rope_scaling_translate.py`) — 이 셀 hf-overrides 의 rope_parameters 가 그 산출이고, 0.31 에서 max_position_embeddings 를 손으로 더해야 했다(W8 · 번역기 교정은 후속으로 남았다 — testlog_26102414 후속 줄).
  - 나머지 8개(R0 · R1 · R4 ~ R7 · R9 · R10 — 측정노드 어휘 · 워치독 선언 · 분류기 · 셀 좌표 이미지 축 · 역루프 통로 · 술어 실행자 · 재발행 · 셀 키 어휘)는 이 셀의 판정 · 수치 · 이름과 무관하다.
- testlog_26102504 §2(항목 1 ~ 5 · 5개 검토) 중 이 셀에 영향을 준 것:
  - 1 — 측정 도구 — vLLM bench(첫 패스 · 판정점 26.92 t/s · 무효) → GuideLLM 0.7.3 재측정 26.01 t/s(R6).
  - 2 — bench 예산 — 이 셀에는 BLOCK 이 없었다(기록 4096 · 상한 11966 · M4).
  - 3(Qwen 싱글 E 초과) · 4(판정기 sweep 신선도 — Solar 에서 발생) · 5(서브 러너 사다리)는 이 셀의 판정 · 수치를 바꾸지 않았다.
- testlog_26102418 §하네스 교정(항목 1 ~ 4 · 4개 검토) 중 이 셀에 영향을 준 것:
  - 2 — 0.31.0 YaRN 의미 변경 — 1M 거부 → max_position_embeddings 동시 오버라이드(W8).
  - 4 — 멀티 서브 통로 · 승인 앵커 실주행 — 서브 compose 부재 → sync_to_sub 배달(W7).
  - 1(싱글 yaml 포트)과 3(런타임 패치 → slave 경로)은 이 셀과 무관하다(이 셀은 런타임 패치가 없다).

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

**토폴로지 · 병렬(V1 · V2)** — 처음부터 멀티 TP=2 · Ray 다. 싱글 TP=1 은 체크포인트 123.53 GiB 가 노드 1대 예산을 가중치만으로 넘는다는 **산술 사전판정**으로 기동 없이 이관됐다(R1 · 시도 0 — 관측된 실패가 아니다). 이 셀 계보에서 다른 TP · 실행기를 시도한 기록은 없다 — 승계값이다.

**컨텍스트(V4 · V12)** — 1,048,576 은 plan_26102412 의 목표("지원 최대 context")이고 체크포인트 네이티브 262,144 에 YaRN factor 4 를 얹은 값이다. 0.29 계보에서는 번역기가 낸 rope_parameters 만으로 서빙됐고(testlog_26091121 §R8), 0.31.0 에서는 max_position_embeddings 를 함께 실어야 했다(W8 · hf-overrides 가 이 셀의 유일한 필요조건 노브다). 값은 처음부터 끝까지 같고, 기동 검증은 시도 2 에서 1,001,130 토큰 needle PASS 까지 올렸다(testlog_26102414 §2 — 측정 기동(시도 5)에서 다시 올리지는 않았다).

**KV(V5 · V6)** — 1M 은 fp8 KV 를 사실상 전제로 첫 plan 이 계획했다(plan_26090918 §3 "1M은 kv=fp8 사실상 전제"). 0.29 계보의 1M resident 셀은 KV 20,480 MiB/노드에서 사살됐고(W6), 이 셀의 yaml 머리 주석은 "이 셀은 KV 를 줄여 재확정한다" 고 적고 15032385536 바이트(14 GiB)를 두었다. 14 GiB 는 바꿔 가며 잰 기록이 없는 손으로 고른 값이다(스윕 0 → 승계값 · V6 근거 "손레버"). 이 클램프에서 엔진이 잡은 용량은 1M 요청 2.01개분이다 — 이 셀은 max-num-seqs 1 이므로 1M 요청 하나의 약 두 배를 잡아 둔 셈이다(음성대조로 일부러 둔 기록은 없다). 엔진은 KV 를 절대 클램프로 잡으면서 gpu-memory-utilization 을 KV 크기 산정에 쓰지 않는다고 스스로 적는다.

> [원문] output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log §L687-688
> (RayWorkerProc pid=401, ip=<node:sub>) (Worker_TP1 pid=401) INFO 10-25 02:14:39 [gpu_worker.py:603] Initial free memory 111.67 GiB, reserved 14.0 GiB memory for KV Cache as specified by kv_cache_memory_bytes config and skipped memory profiling. This does not respect the gpu_memory_utilization config. Only use kv_cache_memory_bytes config when you want manual control of KV cache memory size. If OOM'ed, check the difference of initial free memory between the current run and the previous run where kv_cache_memory_bytes is suggested and update it correspondingly.
> (EngineCore pid=1036) INFO 10-25 02:14:39 [kv_cache_utils.py:2464] GPU KV cache size: 2,112,392 tokens, Maximum concurrency for 1,048,576 tokens per request: 2.01x

해설: 앞 줄은 서브 rank 의 KV 예약(14.0 GiB · 프로파일링 생략 · gmu 미반영 경고)이고 뒷 줄이 그 클램프에서 엔진이 잡은 토큰 용량이다. 엔진은 mamba 상태 페이지에 맞추려고 attention 블록을 3136 토큰으로 올렸다(lite_engine L665).

**호스트 메모리 예산(선언)** — 측정 기동(시도 5)의 예산 선언 행은 아래와 같다. 이 값은 **선언**이다(측정 아님). 산식은 `floor = mem_total − weights − kv − overhead`(budget_preflight.py 머리 주석)이고 이 셀은 124608 − 63266 − 14336 − 24800 = 22206 MiB 다. weights_mib 63266 은 엔진이 잰 노드당 적재 61.93 GiB(lite_engine L655 · L663 — 양 rank)와 나란히 놓을 선언값이고, overhead 24800 은 0.29 캠페인부터 동결된 선언이다(testlog_26091121 §비-measured 셀 "동결 overhead=24,800"). 다섯 시도 모두 같은 선언값(floor 22206 MiB)을 썼다(01 §1.4 원장 표).

> [원문] docs/logs/main/events/2026-10.jsonl §L643-643
> {"expires_epoch": 1792900326, "floor_mib": 22206, "kind": "budget_declare", "kv_mib": 14336, "label": "smoke-q38fn31-nv4-1m-tp2", "mem_total_mib": 124608, "overhead_mib": 24800, "source": "blackbox_session", "ts": "2026-10-25T01:52:06Z", "weights_mib": 63266}

관측 쪽은 측정 중 시스템 RAM 점유 main 93.6 GiB(77%) · sub 90.3 GiB(74%)다(bench_report_26102511 §lite 지표). 기동 중에는 메인 워치독이 시도 2 ~ 5 마다 선언 1분 20초 뒤 `watchdog_highrate_hold` 를 남겼다(행의 `rate_mib_s` 가 `max_rate_mib_s` 20768 을 넘고 `legacy_rule_would_trip=true` 로 기록됐으나 사살 행은 없다 — 01 §1.4 원장 표).

**실행 모드 · 배치(V7 · V9 · V10)** — enforce-eager true 는 0.29 계보의 cudagraph 캡처 피크 사살(W5 · MTP k=3 · gmu 0.85 · KV 20 GiB 조건)에서 온 승계다 — 이 셀 조건에서 cudagraph 를 시도한 기록은 없다(02 Q3). max-num-seqs 1 은 1M 요청 하나만 받는 손으로 고른 값이고(스윕 0), 이 값 때문에 full 스윕의 동시성 레벨 2 · 4 · 8 · 16 이 걸러져 부하 곡선이 레벨 1 하나뿐이다(sweep_index.json `bench_knobs.knobs.levels.filtered_out` — 03 §3.2). max-num-batched-tokens 2048 은 0.29 res 후속 캠페인의 확정 조건(262k)에 있던 값이고(perf_26091122 §4 정정 박스) 이 셀에서 바꿔 잰 기록은 없다.

**gmu · PLE · async(V3 · V11 · V8)** — gpu-memory-utilization 0.80 은 0.29 의 262k resident 셀에서 0.85 → 0.80 으로 낮춰 로드 피크 여유를 연 값이다(testlog_26091304 §3.2 — 그 셀에서는 tuned · 이 셀에서는 승계 · 0.29 1M res 셀은 0.85 였다). 위 발췌대로 이 셀의 엔진은 KV 크기 산정에 이 값을 쓰지 않는다 — gmu 가 이 셀의 로드 피크에 어떤 몫을 했는지는 대조가 없어 미결이다(budget_preflight.py 머리 주석도 그 기전을 미확정으로 적는다). engram-config cpu_offload false 는 PLE 를 장치에 상주시키는 선택이다(R3 · 측정 기동 L631 `weight_device=cuda:0, pinned=False`). async-scheduling false 는 엔진 인자로 넘어가지 않았다(M3).

**판정 산정 입력** — spec decode 를 선언하지 않았으므로 루브릭은 accept_len 1.0 을 쓴다(선언값 · 측정 아님 · 03 §3.1). E=32.5 는 이 세션이 고른 외부 레퍼런스다(testlog_26102504 §1) — 조건 대조는 03 §3.5.

**버티는 기전 대 다른 형상**
- 0.29.0rc6 NVFP4 · resident · 1M(`nv4-bf-1m-res` · `nv4-f8-1m-res`) 대 이 셀 — 다른 것(그 셀 트리플렛 `output/multi/configs/nv4-{bf,f8}-1m-res.yaml` · 계보 표 밖): vLLM 0.29.0rc6 → 0.31.0 · KV 21474836480(auto 와 fp8 둘 다) → fp8 15032385536 · MTP k=3 → 없음 · gmu 0.85 → 0.80 · max-num-seqs · max-num-batched-tokens 미설정(엔진 기본값) → 1 · 2048 · enforce-eager 는 둘 다 true — 그 셀들은 로드 중 서브 MemAvailable 이 memwatch 절대 문턱 아래로 내려가 사살됐고 이 셀은 TRIP 0 — 어느 변경이 피크를 낮췄는지는 대조가 없다 — 미결 — testlog_26091121 §비-measured 셀 · testlog_26102414 §1.
- 0.29 262k resident(KV 20,480 → 8,192 → +배치 억제 → +gmu 0.80) — 관측: KV 회수분을 로드 피크가 되먹었고 gmu 를 낮춘 것이 여유(+5,952 MiB)를 열었다 — 관측은 확정(4단 대조) · 기전(gmu 가 총량 cap 처럼 작용 · 다른 버퍼가 KV 자리를 채움)은 가설(강등 — 총량 cap 코드가 소스에 없다는 2026-09-14 정정 · vllm-recipe-explorer `references/kv-clamp.md` §1 · budget_preflight.py 머리 주석) · 1M 으로의 전이는 미결 — testlog_26091304 §3.2.
- FP8 · resident · 1M(0.31.0 · 자매 셀 q38fn-fp8-1m-tp2) — 가중치가 노드당 88,464 MiB(PLE 상주 포함)로 NVFP4 보다 크다 — 예산 산식에서 arm 상한이 −6,064 MiB 라 기동 전 차단(로드 0초) — 확정(산술 선판정 · 기동 대조 없음) — testlog_26102414_q38fn_fp8 §근거.
- 같은 NVFP4 · 1M 의 PLE mmap 형상(동치 시험 셀 q38fn31-nv4-mmap-1m-tp2 · 런타임 플러그인) — 이 셀과 PLE 백엔드 · 셀 env(VLLM_PLE_MMAP=1 · RUNTIME_PATCH_STEM) · 런타임 패치 유무가 다르다 — PLE 테이블을 재료화하지 않고 디스크에서 읽어 상주 메모리가 줄었다고 계보 밖 문서(testlog_26102500 §2)가 적는다 · 이 셀의 성능과 나란히 잰 대조는 계보 안에 없다 — 미결 — 01 §1.1 · testlog_26102420 §3.

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- 0.31 에서 hf-overrides 에 rope_parameters(YaRN)만 싣고 1M 을 요청하지 말 것 — W8(`_get_and_verify_max_len` 이 factor 를 재적용하지 않아 derived 262144 로 거부) — `text_config.max_position_embeddings` 를 확장값으로 함께 싣고 기동 `non-default args` 와 `Using max model len` 줄을 확인한다.
- 길이 거부를 `VLLM_ALLOW_LONG_MAX_MODEL_LEN=1` 로 넘기지 말 것 — R4(길이 검사만 끄고 YaRN 이 적용되지 않을 위험) — 오버라이드로 연 뒤 needle 로 1M 근방 검색을 확인한다.
- 0.29 계보에서 깨진 `moe-backend` 명시(triton · cutlass)를 근거 없이 다시 넣지 말 것 — W3 · W4(0.29.0rc6 + 빌드 패치 60 조건의 fail-loud · 가드 충돌 · 0.31.0 에서는 명시해 본 기록이 없다) — 이 셀처럼 비워 두고(auto) 엔진 로그 `NvFp4 MoE backend out of potential backends` 줄로 실제 선택과 후보를 본다(이 셀 = FLASHINFER_CUTLASS · 후보에 VLLM_CUTLASS 포함).
- 0.29 의 "resident 는 실패하는 축" 결론을 옮기지 말 것 — M1(KV 과잉 · gmu 천장을 가르지 않은 판정이었다) — 예산 산식으로 floor 를 먼저 계산하고 KV 클램프 · gmu · 배치를 함께 본다.
- KV 클램프만 계속 내려서 resident 로드 사살을 풀려고 하지 말 것 — R5(262k 셀에서 회수한 KV 를 로드 피크가 거의 그대로 되먹었다) — gmu 천장 · 배치 레버를 함께 대조한다.
- 예산 선언 없이 기동하지 말 것 — W2(정상 차단) · W6(선언이 있어도 로드 피크가 memwatch 문턱을 넘으면 사살) — `budget_preflight.py` 로 floor · arm 상한을 먼저 보고 선언 뒤 기동한다.
- GB10 에서 PLE CPU offload(PinnedHost)로 메모리를 줄이려 하지 말 것 — R3(호스트와 GPU 가 한 풀이라 절감 0) — 상주를 줄이려면 디스크 mmap 경로(다른 셀)를 쓴다.
- 체크포인트 크기를 NAS `du` 로 재지 말 것 — testlog_26102406 §근거 1(du 248G 대 safetensors total_size 123.53 GiB) — `model.safetensors.index.json` 의 `metadata.total_size` 나 엔진 로그 `Checkpoint size` 줄을 쓴다.
- needle 을 thinking on · 작은 max_tokens 로 재고 FAIL 을 검색 실패로 읽지 말 것 — M2 — thinking off · temperature 0 으로 재거나 출력 예산을 충분히 준다.
- yaml 의 `async-scheduling: false` 가 비동기 스케줄링을 껐다고 믿지 말 것 — M3(엔진 non-default args 에 없다) — 필요하면 러너 명령줄 `--no-async-scheduling` 으로 넣고 기동 인자 줄에서 확인한다.
- `sweep_bench.sh` 를 `--tool` 없이 돌려 full 이라 부르지 말 것 — R6(기본 도구가 vllm 이라 첫 패스 무효) — `--tool guidellm` 을 명시하고 리포트 측정 구성 표의 bench_tool 을 확인한다.
- 기동 중 60초마다 찍히는 `No available shared memory broadcast block found` 를 hang 으로 읽고 죽이지 말 것 — 01 §1.4 원장 표 #30(이 측정에서 12회 · 11분 · 같은 창에 flashinfer JIT `fused_moe_120` 빌드 줄) — 그 뒤 KV 예약 · autotune · `Starting vLLM server` 까지 기다린다(이 셀 선언 → 서버 기동 23분 23초).
- 엔진 기동 배너의 `0.31.1.dev0+gdb9527a46.d20261014` 를 빌드 입력으로 적지 말 것 — 00 §0.4(빌드 입력은 VLLM_REF=v0.31.0) — 재현 좌표는 릴리스 태그 v0.31.0 또는 40자 SHA 다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

아래 물음들은 계보 문서와 측정 산출물이 아직 답하지 못한 것이다. Q1 · Q2 · Q3 은 이 지도를 다른 환경으로 옮길 때 먼저 걸리는 것이다.

```hint-event
id: Q1
kind: open-question
물음: 이 셀의 decode 26.01 t/s 가 E 32.5 에 못 미친 것은 형상(eager · spec off · 1M · KV fp8 · TP=2 인터커넥트) 중 무엇 때문인가 · 그 E 는 이 셀과 같은 조건인가
현재상태: 판정은 REFUTE(ratio 0.8 · 권한 weak · 서빙 실패 아님)이다. 판정 testlog 는 E 를 이 세션이 고른 외부 레퍼런스라고만 적는다. 같은 값 32.5 를 res 후속 캠페인은 NVIDIA 포럼 1 Spark median(PLE mmap)으로 적었고 싱글 사전판정 testlog 도 1 Spark 사례를 PLE mmap 전제로 적었다 — 이 셀(2노드 · resident)과 노드 수 · PLE 방식이 다르다(같은 출처인지는 이 셀 문서가 말하지 않아 추론). 형상 요인을 하나씩 바꾼 대조는 없다
다음관측: 같은 이미지 · 같은 셀에서 cudagraph(enforce-eager 해제) · MTP 를 하나씩 켠 판정점 decode 와 262k 로 줄인 같은 구성의 decode
출처: [testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §1. 결과표, testlog_26091304_qwen38fn_res최소조건_262k재발행_판정 §2, testlog_26102406_q38fn_nv4_싱글_사전판정_불가_멀티이관 §근거 (전부 관측·산출물 인용)]
```

```hint-event
id: Q2
kind: open-question
물음: 이 형상의 동시성 부하 거동은 어떤가
현재상태: max-num-seqs 1 때문에 full 스윕 프리셋 레벨 2 · 4 · 8 · 16 이 걸러져 레벨 1 만 쟀다(결손 HINT_MISSING_SWEEP_LEVELS). 엔진 KV 용량은 1M 요청 2.01개분이라 짧은 요청 여러 개는 들어갈 여지가 있으나 재지 않았다
다음관측: max-num-seqs 를 올린 같은 구성에서 짧은 요청(입력 1024) 동시성 스윕
출처: [output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/sweep_index.json, output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log]
```

```hint-event
id: Q3
kind: open-question
물음: 이 셀 조건(MTP 없음 · gmu 0.80 · KV 14 GiB · max-num-seqs 1)에서 cudagraph 캡처가 여전히 호스트 사살을 부르는가
현재상태: eager 는 0.29 계보의 캡처 피크 사살(MTP k=3 · gmu 0.85 · KV 20 GiB 조건)에서 승계됐고 이 셀 조건으로 cudagraph 를 시도한 기록은 없다 · 엔진은 eager 때문에 torch.compile · CUDAGraph · JIT warmup 을 끈다고 적는다(lite_engine_head L82)
다음관측: 예산 선언 하에 enforce-eager 를 뺀 같은 구성의 기동에서 캡처 구간 MemAvailable 최저값과 판정점 decode
출처: [testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정, output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log]
```

```hint-event
id: Q4
kind: open-question
물음: 1M 에서 YaRN 위치 보간이 의도대로 적용되는가 — 길이 검사만 넘긴 것이 아닌가
현재상태: 경험적 증거만 있다 — needle 이 네이티브 262,144 를 넘는 400,530 부터 1,001,130 토큰까지 PASS(testlog 는 이것을 YaRN 유효 증거로 적고 mrope × YaRN 합성은 번역기의 unknowns 라고 적는다). 측정 기동 로그의 transformers 경고 `Unrecognized keys in rope_parameters for 'rope_type'='default'` 는 체크포인트 config 의 rope_type 이 default 이기 때문으로 보이며(오버라이드 전 config 파싱 · 추론) YaRN 적용 여부를 말하지 않는다. 회전 임베딩 클래스나 스케일을 찍는 엔진 줄은 찾지 못했다
다음관측: 엔진 안에서 rotary embedding 모듈 타입과 scaling factor 를 출력하거나 같은 프롬프트의 원 컨텍스트 구간 logprob 를 YaRN 유무로 대조
출처: [testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §2. needle(MAGNOLIA-7734 · 증분 프리필 · thinking off · temperature 0), output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log]
```

```hint-event
id: Q5
kind: open-question
물음: 서브 노드의 빌드 원장 동일성과 측정 창의 서브 사건은 어떤가
현재상태: attestation 은 vLLM SHA · torch · driver 를 양 노드 equal 로 관측했으나 build_ledger 축은 sub_identity_sha256 이 null 이라 unobservable 이다. 서브 블랙박스 원장은 2026-09-11 뒤 행이 회수되지 않아 측정 창의 서브 선언 · 사살은 관측 범위 밖이다(없음이 아니다)
다음관측: 서브 이미지의 /opt/easy-vllm/build_ledger.json 회수(문서 기반 · A2A 지시)와 서브 원장 2026-10 행 회수
출처: [output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json, docs/logs/main/events/2026-10.jsonl]
```

```hint-event
id: Q6
kind: open-question
물음: 원장의 기동 시도 3(2026-10-24T12:22:03Z ~ 12:47:59Z)은 무엇이었나
현재상태: 이 셀 라벨로 선언 · 갱신 1회 · 정상 닫힘이 기록됐으나 그 창의 엔진 로그 · 측정 산출물 · 문서 대응이 계보에 없다
다음관측: 그 시각의 컨테이너 로그나 세션 기록 확인
출처: [docs/logs/main/events/2026-10.jsonl]
```

```hint-event
id: Q7
kind: open-question
물음: 기동마다 11분 이어진 shm_broadcast 대기는 flashinfer JIT 빌드인가 · 캐시를 유지하면 줄어드는가
현재상태: 측정 기동에서 02:02:08 에 양 rank 가 flashinfer JIT `fused_moe_120` 빌드를 시작했고 02:02:46 ~ 02:13:46 에 60초 대기 경고 12회가 이어졌다. autotune 은 `Saved 32 configs ... (32 new, 0 from previous config)` 로 끝나 이전 캐시를 쓰지 않았다. 대기가 JIT 빌드 때문이라는 것은 시각 일치에 의한 추론이다
다음관측: JIT · autotune 캐시 디렉터리를 컨테이너 밖에 유지한 재기동에서 대기 경고 횟수와 선언 → 서버 기동 시간
출처: [output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_q38fn31-nv4-1m-tp2.log]
```

```hint-event
id: Q8
kind: open-question
물음: 비동기 스케줄링 켜짐이 이 셀의 decode 에 영향을 주는가
현재상태: 이 측정은 엔진 기본 규칙으로 비동기 스케줄링이 켜진 상태였다(M3 · 이미지 소스 규칙 + 비활성 경고 없음) · yaml 의 false 는 효과가 없었다 · 동기 스케줄링으로 잰 대조는 없다
다음관측: 러너 명령줄에 --no-async-scheduling 을 둔 같은 구성의 판정점 decode
출처: [output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log, q38fn31-nv4-1m-tp2.yaml]
```

현행 측정 등급으로는 이 측정이 full 정의를 충족한다(03 §3.5 사실 블록 · GuideLLM × 반복 3) — 열린 물음이 아니다. 상위 vLLM 에서 불필요해질 자체 패치는 이 셀에 없다(빌드 · 런타임 패치 0).
