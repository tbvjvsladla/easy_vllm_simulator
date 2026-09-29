# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-09-29T05:52:10Z | sweep_index.generated_utc(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce) |
| `accept_len` | 2.015789473684211 | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/verdict.json(output · verdict_rule 출력) measured_accept_len |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26092914_deepseek-v4-flash-0731_GB10_0.29.0.md)) |
| `decode_tps_conc1` | 30.95 | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 26.61 | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-09-29T05:52:10Z | — |
| `lite.cold_ttft_ms` | 7262.8 | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce) |
| `lite.engine_kv_gib` | 10.0 | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce) |
| `lite.engine_kv_tokens` | 1941478 | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce) |
| `lite.engine_reserved_total_gib` | 89.04 | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce) |
| `lite.engine_weights_gib` | 79.04 | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce) |
| `lite.gen_tps` | 28.13 | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce) |
| `lite.kv_gib` | 10.0 | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce) |
| `primary_source` | expected_achievable(roofline×MBU) | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 31.31 | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 0.989 | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | explore | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | true | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce · output) | — |

**`levels`**(5행 · 출처 sweep_index(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 2.015789473684211 | 16 | 30.95 | 31.09 | 0 | 29.751085767558976 | 1 | true | 31.243207885842807 | 32.30905719101429 | 676.0509014129639 |
| 2 | ok | 2.015789473684211 | 17 | 16.39 | 16.58 | 0 | 30.34367841832778 | 2 | true | 33.211972182417334 | 61.008185148239136 | 8191.826105117798 |
| 4 | ok | 2.015789473684211 | 18 | 8.26 | 8.89 | 0 | 30.50840508704092 | 4 | true | 32.474816698111816 | 121.095004491508 | 23352.819442749023 |
| 8 | ok | 2.015789473684211 | 18 | 4.11 | 5.11 | 0 | 29.883731580248067 | 8 | true | 32.94057371458004 | 243.17647516727448 | 54261.29627227783 |
| 16 | ok | 2.015789473684211 | 18 | 3.62 | 3.46 | 0 | 30.45504326913871 | 16 | true | 32.03934361559939 | 276.32371708750725 | 63842.04173088074 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26092914_deepseek-v4-flash-0731_GB10_0.29.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 30.95 | 31.24 | 166.35 | 676.05 | 29.75 | 16/0 |
| 2 | 16.39 | 33.21 | 176.83 | 8191.83 | 30.34 | 17/0 |
| 4 | 8.26 | 32.47 | 172.90 | 23352.82 | 30.51 | 18/0 |
| 8 | 4.11 | 32.94 | 175.38 | 54261.30 | 29.88 | 18/0 |
| 16 | 3.62 | 32.04 | 170.58 | 63842.04 | 30.46 | 18/0 |

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
| `source` | bench_report(bench_report_26092914_deepseek-v4-flash-0731_GB10_0.29.0.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `explore`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `a21e66eb26e5` 2026-09-23T06:53:32Z | 41f20c893cbf..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@a21e66eb26e5` | `git show a21e66eb26e551a8803750cdad06ffc03bbd352d:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `434fa6740831` 2026-09-22T16:32:50Z | 41f20c893cbf..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@434fa6740831` | `git show 434fa6740831c1a3ef91131bd8f13abf95ebed2a:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `fcb0754fffe1` 2026-09-23T01:56:11Z | 41f20c893cbf..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@fcb0754fffe1` | `git show fcb0754fffe107962034fec748ed8823756c93b6:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `692297be56a3` 2026-09-14T13:23:51Z | 41f20c893cbf..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@692297be56a3` | `git show 692297be56a38ed79ff6521e31c6586bd153437c:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

이 run 의 full 스윕은 `sweep_bench.sh` 가 선행 1회 `lite_bench.sh`(`vllm bench serve`)를, 레벨마다 `run_bench.sh`(GuideLLM)를 부른 결과다(위 스냅샷 표). 스윕을 부른 드라이버의 **호출 원문은 계보에 없다** — `broad_search.sh` 가 드라이버 후보라는 것은 스냅샷 표의 '미검증' 그대로다. 실행한 명령 줄 원문도 계보 문서에 없다 — 아래는 측정 시각 이전 마지막 커밋의 도구 원문이고, 01 §1.4 bench 단계의 두 줄은 lite JSON 에서 재구성한 명령이다. 요청 엔드포인트는 sweep meta `bench_endpoint` `/v1/chat/completions` 다(09-28 과 같다).

스윕 기본값(레벨 · 입출력 길이 · 프롬프트 수 · 워밍업) — 이 run 은 레벨 · 길이를 덮어쓰지 않았다(3.1 levels 1,2,4,8,16 · bench_report 측정 환경 스냅샷 `입력 길이(sweep)` 1024):

> [원문] sweep_bench.sh@a21e66eb26e5 §L55-55
> TOPO=""; SERVE_PLANE="docker"; HOST_ENDPOINT=""; CLIENT_VLLM=""; LEVELS="1,2,4,8,16"; ILEN=1024; OLEN=256; NPROMPTS=16; WARMUPS=2; VLLM_VER=""; DRYRUN=0; REASSEMBLE=0

full 레그(GuideLLM · 동시성 스윕) — 도구가 정한 인자는 `ignore_eos:true` · 동시 스트림 프로파일 · 합성 텍스트 데이터 · 요청 수 상한 · 오프라인 토크나이저:

> [원문] run_bench.sh@434fa6740831 §L380-386
>     -e HF_HUB_OFFLINE=1 -e TRANSFORMERS_OFFLINE=1 \
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \

lite 레그(`vllm bench serve` · cold 1 / warm 3 요청) — 입력 512 · 출력 128 · range-ratio 0 · 동시성 1 · `--ignore-eos` · `--trust-remote-code`(temperature 인자는 이 판본 lite 명령에 없다 — 도구가 정하지 않았다):

> [원문] lite_bench.sh@fcb0754fffe1 §L136-141
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url $BASE_URL --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

조건 요약:
- 도구 · 버전: GuideLLM 0.7.3(full 레그 · 3.3 표 · 도구 이미지 digest 는 09-28 과 같다 — sweep meta `bench_tool_image_digest`) · lite 는 서빙 이미지 안 `vllm bench serve`.
- 입출력: full 1024 / 256 토큰(합성 텍스트) · lite 512 / 128(인자 · 01 §1.4 재구성 명령과 같다 — 같은 곳 주석의 595 / 128 은 JSON 합계 ÷ 완료 수로 낸 실측 토큰이며 FACT 는 이를 chat 템플릿 포함으로 표기한다).
- 레벨 · 반복: 동시성 1/2/4/8/16 · 레벨마다 같은 serve 에 반복 3(warm-rerun · 3.3 `repeats` 3 · 레벨 run 시도 합 15 · bench_report 반복 축) · 대표값은 첫 완주 run · lite 선행 레그 1회.
- warm/cold: 스윕은 워밍업 있음(`WARMUPS=2`) · cold TTFT 는 lite cold 단일 요청(warmup 0 · bench_report lite 지표).
- spec: on(dspark k=7) · accept_len 2.015789473684211(3.1) — lite warm 레그 실측을 판정 레벨로 승계한 값(verdict.json accept_len_evidence · GuideLLM 레그에서 잰 값이 아니다). lite cold 레그는 2.01587(01 §1.4 bench 재구성 주석).
- 서빙 구성: 트리플렛 yaml 그대로 — `enable-auto-tool-choice: true` 포함(측정 실행 엔진 non-default args · engine_v7 L62). 09-28 판과 달리 측정 구성과 실린 yaml 이 같다.
- 측정 노드: cluster(메인 master 엔드포인트 · TP=2 두 노드) · 측정 시각 2026-09-29T05:13:42Z → 2026-09-29T05:52:10Z(01 §1.4 bench 단계).
- reload 없음 · request-rate inf.

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:63` 원문 그대로):

> full 계측(`lite ∪ GuideLLM × 반복 ≥3`)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26092914_deepseek-v4-flash-0731_GB10_0.29.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26092914_deepseek-v4-flash-0731_GB10_0.29.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

**비교 대상**(강한 일치 키 = model · gpu_model · vllm_version · quantization · topology · tensor_parallel_size — 인증서 규칙 · 이 run 은 인증서가 없어 sweep meta 로 대조했다)

- 같은 셀의 앞선 판(09-28 · camp-26092808 · `enable-auto-tool-choice` 없는 구성) · 동시성1 32.2 t/s · 강한 키 6개 같음 · 소프트 지문 대조(두 sweep_index meta 전 키): driver_version 580.173.02 · cuda_version 132 · image_tag · **image_digest**(`sha256:a2c4ca49…` 양쪽) · max_model_len 1048576 · max_num_seqs 1 · kv_cache_memory_bytes 10737418240 · kv_cache_dtype fp8 · gpu_memory_utilization 0.85 · moe_backend humming · enforce_eager false · attention_backend NA · ple_mode resident · bench_tool guidellm 0.7.3(도구 이미지 digest 도 같음) 가 **전부 같다** · 부하 조건(1024 / 256 · 레벨 1~16 · 레벨당 요청 16~18 · spec dspark k=7 · GuideLLM · 엔드포인트 chat)도 같다 · 요청당 토큰 구성 확인: 동시성 1 의 총 tok/s ÷ 출력 tok/s 가 166.35 / 31.24 대 172.08 / 32.32 로 양쪽 약 5.32 · **다른 것은 소프트 지문 밖**의 서빙 플래그 `enable-auto-tool-choice`(엔진 non-default args 대조 — engine_v7 L62 대 engine_roce 로그) 하나와 측정 시각이다(로그가 echo 하지 않은 env 와 호스트 상태는 대조 밖) · **비교 가능(같은 형상 · 서빙 플래그 1개 다름 — 소프트 지문은 그 차이를 싣지 않는다)**.
- 09-09 대조군 `E1-combo`(= 셀 `e-1m-kvfp8-e1combo` · 같은 레시피 · 플래그 없음) · 동시성1 30.54 t/s · 강한 키 6개 같음 · 소프트 지문: `image_digest` **다름**(09-09 `sha256:4c9ab74a…` · 이 run `sha256:a2c4ca49…`) · 나머지 좌표 키(quant · max-len · kv dtype · TP · image_tag · moe · attention · gmu · seqs)는 sweep_map_26090912_e1m_levers 좌표와 같고 bench_tool guidellm 0.7.3 도 같다 · 그 좌표에 없는 driver · cuda · ple_mode 등은 이 문서로 대조하지 못했다 · 인증서 키 밖에서 커널(6.17 계열 → 7.0.0-1019) · NCCL env 일부(M1) · tool-choice 플래그가 다르다 · **비교 가능(형상 비교 · stale — image_digest 불일치)**. 두 쪽 `driver_version` 이 같더라도 둘 다 manifest 선언을 옮긴 값이다(M3).
- 외부 레퍼런스 · 없음 · E-search 상태 no · **비교 대상 없음** — bench_report_26092914 §판정.

**같은 셀의 앞선 판과 레벨 전부**(값은 두 원천 글자 그대로 — 이 측정 = 3.2 부하 곡선 · 이전 = bench_report_26092809 부하 스윕 곡선 · 비 = 이 측정 ÷ 이전 · 소수 셋째 자리 반올림):

| 동시성 | 이 측정 | 이전 측정(09-28 · 플래그 없음) | 비 |
|---|---|---|---|
| 1 | 30.95 | 32.2 | 0.961 |
| 2 | 16.39 | 16.2 | 1.012 |
| 4 | 8.26 | 8.53 | 0.968 |
| 8 | 4.11 | 4.35 | 0.945 |
| 16 | 3.62 | 3.69 | 0.981 |

동시성 1 의 차이(비 0.961)는 09-28 판정점 재현 밴드 3.15% 보다 커서 "같은 범위 재현" 이라 부르지 않는다. 반대로 이 run 의 동시성 1 반복은 30.95 / 33.32 / 35.39(밴드 13.37% · bench_report_26092914 §반복 축)로 09-28 의 32.2 가 그 사이에 있고, 대표값은 첫 완주 run(이 run 에서 가장 낮은 값)이다. 두 측정 모두 레벨 2 이상에서 출력 tok/s 는 31~34 대로 평탄하다(max-num-seqs 1). acceptance length 는 비교 조건이 아니라 출력이다 — 두 값은 **둘 다 lite warm 레그**의 실측을 판정 레벨로 승계한 것이다: 이 run 2.015789473684211 · 09-28 2.238372093023256 · 비 0.901(소수 셋째 자리). 처리량 비 0.961 은 accept_len 비와 크기가 달라 그 비만으로는 설명되지 않고, accept_len 이 GuideLLM 레벨이 아니라 3요청 lite 레그의 값이라 레벨 처리량과의 연결도 가를 수 없다 — **판정 불가**. 플래그가 API 층이라 decode 경로에 닿지 않는다는 것은 판정 문서의 추정이다(Q4). lite warm gen t/s 도 28.13 대 31.82 로 이 run 이 낮다(bench_report 두 판의 lite 지표).

**판정 등급.** 이 run 의 판정은 verdict.json · measurement.verdict **PASS** · 권위 **explore**(문턱을 세우지 않는 서술 판정) · primary = expected_achievable(roofline×MBU) 31.31 · floor 26.61 · ratio 0.989 · tolerance 0.15 · 판정 입력의 accept_len 은 lite warm 레그 실측의 기계 승계값(손 승계 아님 · GuideLLM 판정 레벨에서 잰 값 아님) — 합격선이 accept_len 을 곱하므로 09-28(floor 29.55 · accept_len 2.238)보다 낮다. 외부 레퍼런스 없이 roofline 만으로 섰다(경고 `E-not-attempted` · Q5). **인증서는 없다** — 인증서는 explicit ∧ PASS 에서만 나온다는 규칙의 비발행 판정이다(3.3 사실 블록 · 결손 아님). 위 사실 블록대로 **현행 full 정의를 충족한다**(반복 3 ≥ 3 · GuideLLM). waiver 아님 · 관측 게재 아님. 재측정이 필요한 조건: 네 환경에서 소프트 지문(driver · cuda · image digest · max-len · kv-bytes · gmu · moe) 중 하나라도 다르면 stale 이다 — 드라이버는 선언이 아니라 관측값으로 대조하고(M3), image digest 와 NCCL 전송(`Using network` 줄)을 먼저 본다. 소프트 지문은 `enable-auto-tool-choice` 같은 API 층 플래그를 싣지 않으므로, 네 구성과 이 측정을 나란히 둘 때는 엔진 로그 non-default args 도 대조한다.
