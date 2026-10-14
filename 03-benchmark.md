# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-14T06:32:45Z | certificate(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) measured_utc |
| `accept_len` | 미기재 | lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`) |
| `bench_tool` | vllm-bench-serve | certificate(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) bench_tool |
| `benchmark_mode` | lite | certificate(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) benchmark_mode |
| `decode_tps_conc1` | 미기재 | lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`) |
| `floor_tps` | 미기재 | lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`) |
| `lite_accept_len` | N/A | certificate(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) lite_accept_len |
| `lite_cold_ttft_ms` | 169.3 | certificate(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) lite_cold_ttft_ms |
| `lite_gen_src` | median_tpot | certificate(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) lite_gen_src |
| `lite_gen_tps_warm` | 24.07 | certificate(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) lite_gen_tps_warm |
| `lite_included` | true | certificate(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) lite_included |
| `lite_kv_gib` | 21.42 | certificate(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) lite_kv_gib |
| `ratio_M_over_primary` | 미기재 | lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`) |
| `rubric_authority` | 미기재 | lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`) |
| `verdict` | OBSERVATION-ONLY | lite-only 셀 — verdict_rule 에 투입되지 않았다(경량 리포트 `inform-only` · 판정 원천 없음 · benchmark_mode=lite · report_kind=lite) |
| `source` | certificate(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) | — |

> **OBSERVATION-ONLY** — 이 태그의 성능 수치는 **관측 게재**다(인증서 없음). baseline·권고로 읽지 마라 — baseline 을 주장하려면 full_benchmark 인증서가 필요하다.
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## lite 지표 — 서빙 성공 직후 스냅샷 (결정론 파싱 · 손저작 ✗)

> 서빙 성공 직후의 **lite 스냅샷**이다 — cold 1회 + warm burst 1회. 동시성 곡선·반복·판정·인증서가 없다.
> 성능 baseline 이 아니며(OBSERVATION-ONLY) 수신자는 자기 환경에서 재측정한다.
> 출처: `bench_report_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.md` (bench_report(lite)) · 표는 리포트의 행을 그대로 옮긴다(재산정 ✗ · `--verify` diff 0).

| 메트릭 | 값 |
|---|---|
| gen tokens/sec (warm) | 24.07 t/s |
| cold-start TTFT | 169 ms |
| GPU VRAM 점유 | 84.8 GiB (89%) |
| KV cache 점유 | 21.4 GiB (22%) |
| 시스템 RAM 점유 | 4.3 GiB (7%) |

## 3.3 측정 구성
<!-- FACT:measurement_config -->
| 키 | 값 |
|---|---|
| `bench_mode` | lite |
| `bench_mode_kind` | declared-lite |
| `downgrade_reason` | 미기재 |
| `bench_tool` | vllm-bench-serve |
| `bench_tool_version` | 미기재 |
| `repeats` | 1 |
| `repeats_completed` | 1 |
| `source` | bench_report(bench_report_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `OBSERVATION-ONLY` — 출처 lite-only 셀 — verdict_rule 에 투입되지 않았다(경량 리포트 `inform-only` · 판정 원천 없음 · benchmark_mode=lite · report_kind=lite) · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `미관측`).

**측정 결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `BENCH_MODE_LITE` | full bench 정의(lite ∪ GuideLLM × 반복 ≥3)를 충족하지 않았다고 **기재된** 측정이다 — 선언된 lite-only 셀이거나 반복 불성립(기계 이벤트)으로 강등된 셀이다. 수치는 lite 스냅샷(또는 강등 셀의 대표 run 1회)이라 산포 추정치·인증서가 없다. 어느 쪽인지·강등 사유는 측정 구성 표(bench_mode_kind · downgrade_reason)가 말한다. bench_mode 를 읽지 못한 측정(미확정·미기재)에는 붙이지 않는다 — 모름을 lite 로 접으면 합성이다(그 사실은 카탈로그 bench_mode 칸이 말한다). |
| `HINT_MISSING_SWEEP_LEVELS` | 부하 레벨이 1개 이하 — 부하 거동을 알 수 없다(경량 리포트는 동시성 곡선을 재지 않는다). |
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 1건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `803064d23d51` 2026-10-06T06:17:10Z | 924ae694d254..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@803064d23d51` | `git show 803064d23d516fc72600077cf65e9f6c2a8775d5:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |

- `lite_bench.sh` — 경량 리포트 셀(lite_bench.sh 가 리포트를 발행한다)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

이 태그에 바인딩된 측정은 **lite 등급**이다 — `lite_bench.sh qwen3-32b --topology single --publish-report`(cold 단일 요청 + warm burst 3 · 동시성 1). 도구가 정한 인자(스냅샷 원문 · cold 레그):

> [원문] lite_bench.sh@803064d23d51 §L175-176
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \

조건 목록 — 도구: `lite_bench.sh`(rev 803064d23d51 · 낶 `vllm bench serve`) · 입력 512 / 출력 128 토큰 · cold = num-prompts 1 · warmup 0 · warm burst = num-prompts 3 · warmup 1 · max-concurrency 1 · request-rate inf · random-range-ratio 0 · ignore-eos · spec decode 끔(미설정) · 측정 노드 main · 측정 시각 2026-10-14T06:32:45Z.

이 태그의 **셀 지식**(N* = bf16 10/2 · fp8 21/5)은 별개 측정이다 — 캠페인 프로브 드라이버(GuideLLM concurrent · 입력 7,000/30,000 · 출력 1,024 · 반복 3 · 쿨다운 ≤55°C)와 KV 산술 상한 증명으로 얻은 것으로, 원시는 캠페인 `probe/qwen3-32b/**`에 있고 판정은 testlog_26101319 §판정 표 · §백필이다. lite 바인딩과는 도구·부하 조건이 달라 나란히 놓지 않는다.

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 1 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | vllm-bench-serve | bench_report(bench_report_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.md) |
| 현행 full 정의 충족 | 아니오 | 측정 구성 bench_mode=lite — full 측정이 아니다 · 판정점 반복 1 < 현행 정의 3 · 측정 도구 vllm-bench-serve — 현행 정의의 full 레그(GuideLLM)가 아니다 |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- 이 태그의 lite 측정(warm 24.07 t/s · 동시성 1) ↔ 같은 셀의 캠페인 프로브 측정(GuideLLM concurrent) — **비교 불가** — 도구·입출력 길이·동시성이 모두 다르다. N* 수치의 정본은 testlog_26101319 §판정 표 · §백필이다.
- 이 측정 ↔ 같은 셀의 이전 lite 측정 — 이 캠페인 최초의 바인딩 lite라 이전 측정 없음.
- 자매 셀(qwen3-4b · muse-glimmer-30b) — 강한 일치 키 `model`이 달라 **무효** — 비교하지 않는다.
- 손 승계 입력 — lite 경로에는 acceptance length · 루브릭 입력이 없다.

**판정 등급** — `OBSERVATION-ONLY`(hint_map_only · lite 등급 인증서): 성능 판정에 투입되지 않은 관측 게재다. 현행 full 정의(`GuideLLM × 반복 ≥3`)를 **충족하지 않는다**(반복 1 · 도구 vllm-bench-serve). baseline으로 읽지 말 것. 이 셀의 30K 상한(bf16 2 · fp8 5)은 측정이 아니라 KV 산술 증명 + 저N 확인의 병기다(02 M2 — 수치의 지위를 구분해 읽을 것).
