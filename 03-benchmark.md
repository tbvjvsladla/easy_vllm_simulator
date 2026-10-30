# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-30T15:54:26Z | sweep_index.generated_utc(output/multi/benchlog/sweep_loading_ds41-load-local-1m) |
| `accept_len` | 1.7579908675799087 | output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) measured_accept_len |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26103100_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.32.0.md)) |
| `decode_tps_conc1` | 22.26 | output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 37.17 | output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-30T15:54:26Z | — |
| `lite.cold_ttft_ms` | 2161.1 | sweep_index.lite(output/multi/benchlog/sweep_loading_ds41-load-local-1m) |
| `lite.engine_kv_gib` | 8.0 | sweep_index.lite(output/multi/benchlog/sweep_loading_ds41-load-local-1m) — 서빙 yaml `kv-cache-memory-bytes: 8589934592`(8.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/multi/configs/ds41-load-local-1m.yaml) |
| `lite.engine_kv_tokens` | 2608092 | sweep_index.lite(output/multi/benchlog/sweep_loading_ds41-load-local-1m) |
| `lite.engine_reserved_total_gib` | 8.0 | sweep_index.lite(output/multi/benchlog/sweep_loading_ds41-load-local-1m) |
| `lite.engine_weights_gib` | 73.96 | sweep_index.lite(output/multi/benchlog/sweep_loading_ds41-load-local-1m) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/multi/benchlog/sweep_loading_ds41-load-local-1m) |
| `lite.gen_tps` | 18.81 | sweep_index.lite(output/multi/benchlog/sweep_loading_ds41-load-local-1m) |
| `lite.kv_gib` | 8.0 | sweep_index.lite(output/multi/benchlog/sweep_loading_ds41-load-local-1m) — 서빙 yaml `kv-cache-memory-bytes: 8589934592`(8.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/multi/configs/ds41-load-local-1m.yaml) |
| `primary_source` | E(external_reference) | output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 43.73 | output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 0.509 | output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | true | output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | REFUTE | output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/multi/benchlog/sweep_loading_ds41-load-local-1m · output) |
| `verdict_reasons` | M(22.26) < 43.73×(1−0.15)=37.17 [E(external_reference)] (ratio 0.509) → 기대 이하 | output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) verdict |
| `source` | 판정 원천(verdict.json) + sweep_index(output/multi/benchlog/sweep_loading_ds41-load-local-1m · output) | — |

**`levels`**(1행 · 출처 sweep_index(output/multi/benchlog/sweep_loading_ds41-load-local-1m · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 1.7579908675799087 | 16 | 22.26 | 22.03 | 0 | 42.139861162971044 | 1 | true | 22.115934004263103 | 44.91893947124481 | 750.4868507385254 |

> **REFUTE** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) verdict.
> 기각 사유: M(22.26) < 43.73×(1−0.15)=37.17 [E(external_reference)] (ratio 0.509) → 기대 이하
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 동시 요청 수를 부하 축으로 표시한다. 값이 있는 행은 원천에 기록된 해당 레벨의 대표 run(첫 완주 run)을 나타낸다.
> 반복 run의 warmup·요청 수·완료 결과는 서로 다를 수 있으므로 원천 기록을 함께 확인한다.
> `★판정점`은 원천이 표시한 판정 지점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다. 인증서 발행 여부는 판정과 루브릭 권한에 따른다.
> 출처: `bench_report_26103100_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.32.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 22.26 | 22.12 | 113.17 | 750.49 | 42.14 | 16/0 |

> ⚠ 레벨이 **하나뿐**이다 — 이 레시피의 부하 거동은 이 hint 로 알 수 없다. 곡선이 필요하면 재측정해야 한다(부재를 성능 판정으로 읽지 말 것).

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
| `source` | bench_report(bench_report_26103100_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.32.0.md) |
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
| `source` | bench_report(bench_report_26103100_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.32.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `REFUTE` — 출처 output/multi/benchlog/sweep_loading_ds41-load-local-1m/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `HINT_MISSING_SWEEP_LEVELS` | 부하 레벨이 1개 이하 — 부하 거동을 알 수 없다(경량 리포트는 동시성 곡선을 재지 않는다). |
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `17959581d8ae` 2026-10-27T07:26:14Z | ede01ab6a835..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@17959581d8ae` | `git show 17959581d8ae97c2e951120dbca09a5a882e3051:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `29cbdbc89a3d` 2026-10-17T02:03:29Z | ede01ab6a835..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@29cbdbc89a3d` | `git show 29cbdbc89a3d694097e94a9f9d6c56dde4bc1ee0:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `d33b0878a6c3` 2026-10-17T13:18:03Z | ede01ab6a835..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@d33b0878a6c3` | `git show d33b0878a6c37dc7d89aec06861ba438ceffb4de:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `17959581d8ae` 2026-10-27T07:26:14Z | ede01ab6a835..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@17959581d8ae` | `git show 17959581d8ae97c2e951120dbca09a5a882e3051:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/multi/benchlog/sweep_loading_ds41-load-local-1m/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 이 선택기는 호출 기록을 조회하지 않는다(정적 소스 기반 후보이며 실제 드라이버 여부는 이 선택기에서 미검증). 별도 호출 관측과 이 판본의 실제 실행 바이트 결속은 구분한다
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

GuideLLM 버전·UTC·실제 완료/실패 수는03.1/03.3의 현재 관측이 권위다. 이번은 synthetic1024/256·openai-chat·C1 warm-rerun3이며 max-num-seqs1이 상위 레벨을 필터했다. 현재 DSpark3의 acceptance는 새 full/lite warm에서 승계한 실측 출력이고 과거131K 값이 아니다.

> [원문] testlog_26103023_DeepSeek_1M_로딩비교_진행 §Full 실제 종결 (2026-10-30T16:00:22Z)
> GuideLLM0.7.3/openai-chat/synthetic1024/256/C1/warm-rerun3, actual client4096MiB다. full 선행 lite pass, 각run16완료/0실패, post-health200/running/oom=false다.

GuideLLM leg의 실제 도구 원문은 아래와 같다. temperature/seed/chat_template_kwargs를 이 호출에서 명시하지 않으므로 다른 leg의 temperature0이나 외부의 thinking off를 현재 조건으로 복사하지 않는다. warmup은 요청 수에서 TOTAL_REQ/비율로 바뀌며 실제 집계 수를 읽는다.

> [원문] run_bench.sh@29cbdbc89a3d §L445-450
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \

lite는 별도512/128 random·range-ratio0·ignore-eos·cold warmup0/warm1이다. 이 호출의 temperature/seed도 원문과 엔진 기본을 구별한다.

> [원문] lite_bench.sh@d33b0878a6c3 §L179-182
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26103100_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.32.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26103100_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.32.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- 같은 셀의 이전 full: 새 ds41-load-local-1m에는 이전 full이 없다. 첫 두 local startup의 lite와 현재 같은 서버의 세 warm rerun을 이전 full로 세지 않는다.
- 이전0.32 i5-032-sm12x:131K·24.04/22.73/23.66t/s와 acceptance1.8461538461538463은 다른 셀의 관측이다. 현1M 및 명시적 모델-root Engram 경로와 조건 전체의 동등 비교를 정립하지 않았으므로 개선율·같은 범위 재현으로 쓰지 않는다(선행 testlog_26103005). 알려진 차이를 하나의 인과로 압축하지 않는다.
- 과거0.30 i5-a-kern: graph·선택 성능 커널 묶음·seq2 및 버전/이미지의 강한·소프트 지문이 다르다. boot4 C1 full34.41/40.83/39.80을 현 eager 결과와 직접 순위 비교하지 않는다(과거 정식 레시피 testlog).
- 저장정책 자매 셀: serving YAML·모델 바이트를 고정해 startup 비교는 가능하다. NAS/main-NFS의 full decode throughput은 이번에 측정하지 않아 대표 local full로 그 처리량 동등을 주장하지 않는다(현재 testlog).
- 외부 직접저자 round35:43.73 prose_longC1 natural finish는 weak 성능 앵커다. graph+10kernel/seq2·batched8192와 현재 eager/minimal/seq1·batched1536/synthetic1024/256, 정확 pack revision 동등 미관측의 차이를 공개한다. round36 Viterbi43.80이나LAIL43.17을 같은 값으로 바꾸지 않는다(현재 pre-full 루브릭 결정).
현재 등급은 lite pass+GuideLLM3회의 full이다. 실제 PASS/REFUTE·floor·ratio·authority는03 FACT의 owner 출력을 그대로 따른다. weak 권한은 explicit 전용 인증서를 발행하지 않는다. 판정과 서빙/유효 측정의 성립은 구별하며 처리량 미달을 숨기지 않는다. acceptance는 측정 결과이므로 다른 부팅과 달라도 그것만으로 비교 조건 불일치라고 하지 않는다. 다만 spec 방식·부하·이미지·context가 바뀌면 새 측정과 전체 지문 대조가 필요하다.
