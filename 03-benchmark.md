# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-28T22:11:36Z | sweep_index.generated_utc(output/single/benchlog/sweep_gpt120v0320_20261028T2143) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_gpt120v0320_20261028T2143/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26102907_gpt-oss-120b_GB10_0.32.0.md)) |
| `decode_tps_conc1` | 35.86 | output/single/benchlog/sweep_gpt120v0320_20261028T2143/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 13.18 | output/single/benchlog/sweep_gpt120v0320_20261028T2143/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-28T22:11:36Z | — |
| `lite.cold_ttft_ms` | 215.0 | sweep_index.lite(output/single/benchlog/sweep_gpt120v0320_20261028T2143) |
| `lite.engine_kv_gib` | 0.44 | sweep_index.lite(output/single/benchlog/sweep_gpt120v0320_20261028T2143) — 서빙 yaml `kv-cache-memory-bytes: 467380993`(0.4 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/gptoss120b-0191-c0.yaml) |
| `lite.engine_kv_tokens` | 8350 | sweep_index.lite(output/single/benchlog/sweep_gpt120v0320_20261028T2143) |
| `lite.engine_reserved_total_gib` | 66.58 | sweep_index.lite(output/single/benchlog/sweep_gpt120v0320_20261028T2143) |
| `lite.engine_weights_gib` | 66.14 | sweep_index.lite(output/single/benchlog/sweep_gpt120v0320_20261028T2143) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_gpt120v0320_20261028T2143) |
| `lite.gen_tps` | 37.6 | sweep_index.lite(output/single/benchlog/sweep_gpt120v0320_20261028T2143) |
| `lite.kv_gib` | 0.44 | sweep_index.lite(output/single/benchlog/sweep_gpt120v0320_20261028T2143) — 서빙 yaml `kv-cache-memory-bytes: 467380993`(0.4 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/gptoss120b-0191-c0.yaml) |
| `primary_source` | expected_achievable(roofline×MBU) | output/single/benchlog/sweep_gpt120v0320_20261028T2143/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 15.51 | output/single/benchlog/sweep_gpt120v0320_20261028T2143/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 2.312 | output/single/benchlog/sweep_gpt120v0320_20261028T2143/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_gpt120v0320_20261028T2143/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_gpt120v0320_20261028T2143/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_gpt120v0320_20261028T2143/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_gpt120v0320_20261028T2143/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_gpt120v0320_20261028T2143 · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_gpt120v0320_20261028T2143 · output) | — |

**`levels`**(5행 · 출처 sweep_index(output/single/benchlog/sweep_gpt120v0320_20261028T2143 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 35.86 | 35.85 | 0 | 26.783473818909886 | 1 | true | 35.94607774440904 | 27.886916883289814 | 306.915283203125 |
| 2 | ok | 미기재 | 16 | 28.27 | 28.32 | 0 | 33.35269946678012 | 2 | true | 56.8779378241405 | 35.37300229072571 | 586.9429111480713 |
| 4 | ok | 미기재 | 16 | 19.8 | 20.46 | 0 | 47.28532398448271 | 4 | true | 68.26789200333032 | 50.514942035079 | 634.5014572143555 |
| 8 | ok | 미기재 | 16 | 13.72 | 14.24 | 0 | 68.94254029965869 | 8 | true | 88.60512994910286 | 72.86049425601959 | 1180.4780960083008 |
| 16 | ok | 미기재 | 16 | 11.9 | 8.79 | 0 | 76.08683343027153 | 16 | true | 97.70327139260839 | 83.99853482842445 | 2227.908134460449 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 동시 요청 수를 부하 축으로 표시한다. 값이 있는 행은 원천에 기록된 해당 레벨의 대표 run(첫 완주 run)을 나타낸다.
> 반복 run의 warmup·요청 수·완료 결과는 서로 다를 수 있으므로 원천 기록을 함께 확인한다.
> `★판정점`은 원천이 표시한 판정 지점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다. 인증서 발행 여부는 판정과 루브릭 권한에 따른다.
> 출처: `bench_report_26102907_gpt-oss-120b_GB10_0.32.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 35.86 | 35.95 | 179.87 | 306.92 | 26.78 | 16/0 |
| 2 | 28.27 | 56.88 | 284.61 | 586.94 | 33.35 | 16/0 |
| 4 | 19.80 | 68.27 | 341.61 | 634.50 | 47.29 | 16/0 |
| 8 | 13.72 | 88.61 | 443.37 | 1180.48 | 68.94 | 16/0 |
| 16 | 11.90 | 97.70 | 488.90 | 2227.91 | 76.09 | 16/0 |

_절삭된 레벨 없음(요청 전 레벨 완주)._

## 3.3 측정 구성
<!-- FACT:measurement_config -->
| 키 | 값 |
|---|---|
| `bench_mode` | full |
| `bench_mode_kind` | full |
| `downgrade_reason` | 미기재 |
| `bench_tool` | guidellm |
| `bench_tool_version` | 0.7.3 |
| `repeats` | 3 |
| `repeats_completed` | 3 |
| `source` | bench_report(bench_report_26102907_gpt-oss-120b_GB10_0.32.0.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:dataset_provenance -->
### 데이터셋 출처

> 리포트 측정 구성 표 · 인증서 소프트 지문이 **기재한** 데이터셋 출처다(파싱만 · 합성 ✗ · `/app/` 밖 호스트 경로는 이름만).

| 키 | 값 |
|---|---|
| `dataset_kind` | synthetic |
| `dataset_source` | 미기재 |
| `dataset_id` | 미기재 |
| `dataset_revision` | 미기재 |
| `dataset_source_sha256` | 미기재 |
| `dataset_column_map` | 미기재 |
| `dataset_preprocess_strategy` | 미기재 |
| `dataset_seed` | 미기재 |
| `dataset_duplicate_sampling` | 미기재 |
| `dataset_tokenizer` | 미기재 |
| `dataset_provenance` | 미기재 |
| `status` | recorded |
| `source` | bench_report(bench_report_26102907_gpt-oss-120b_GB10_0.32.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_gpt120v0320_20261028T2143/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `b799890f7e49` 2026-10-27T07:28:50Z | 273ace983213..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@b799890f7e49` | `git show b799890f7e4996fecf84b3999b9b0100909f80b6:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `1629609b34df` 2026-10-16T21:44:18Z | 273ace983213..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@1629609b34df` | `git show 1629609b34dfe6851338eb7b8353411a1016924d:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `a7083a8f933d` 2026-10-17T13:17:27Z | 273ace983213..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@a7083a8f933d` | `git show a7083a8f933d98cd6d739fdac838ebbe3e3b7d3f:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `b799890f7e49` 2026-10-27T07:28:50Z | 273ace983213..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@b799890f7e49` | `git show b799890f7e4996fecf84b3999b9b0100909f80b6:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_gpt120v0320_20261028T2143/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 이 선택기는 호출 기록을 조회하지 않는다(정적 소스 기반 후보이며 실제 드라이버 여부는 이 선택기에서 미검증). 별도 호출 관측과 이 판본의 실제 실행 바이트 결속은 구분한다
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

측정 형상은 위 FACT와 원시 결과를 따른다. 실제 명령의 완전한 argv는 계보 문서에 남지 않았으며 아래 도구 원문은 스윕 경계만 설명한다.

> [원문] sweep_bench.sh@b799890f7e49 §L4-6
> # 편지 패턴 B.4: 단일 running serve 에 **client-load 만** 변화(동시성 레벨 스윕) → 부하별 곡선.
> #   **reload 0** — 벤치마커 "기동 안 함" 불변식 보존(config-space=batch×maxlen reload 는 Max 모드/explorer 소관).
> #   판정점(동시성=1)을 스윕이 포함 → verdict 는 그 레벨을 재사용(재측정 0).

GuideLLM 0.7.3, openai completions, 선언 입력 1024 출력 256 요청 16 warmup 2, 각 레벨 반복 3, sub, spec off. 일부 실제 실행은 그 선언과 다르다.

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/skills/adversarial-benchmark/SKILL.md:33` 원문 그대로):

> `full = lite ∪ GuideLLM × 반복 ≥3`

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26102907_gpt-oss-120b_GB10_0.32.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26102907_gpt-oss-120b_GB10_0.32.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm · sweep_index lite_verdict=pass · 측정 레벨 5개 모두 완주 반복 ≥3 |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

이 결과는 full·weak PASS이며 외부 E가 없어 roofline 기준이다. 다른 runtime이나 하드웨어와 속도를 순위로 비교하지 않는다. 동시성 2의 run2와 run3은 각각 JSONDecodeError 1건과 post-health 200을 기록했고 근본 원인은 미해결이다. 동시성 16의 run2와 run3은 warmup 0과 성공 18건이며 선언은 warmup 2와 요청 16이다. 관측 입력 1025도 함께 확인한다.
