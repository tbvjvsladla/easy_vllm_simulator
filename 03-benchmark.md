# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-05T07:28:55Z | certificate(benchmark_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.yaml) measured_utc |
| `accept_len` | 2.053475935828877 | output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_warm_i5-a-kern.json spec_decode_acceptance_length(warm 레그 · 원문) · cold 1.9104477611940298 — lite 관측(판정 입력 아님 · lite-only 셀은 verdict_rule 에 투입되지 않았다) |
| `bench_tool` | vllm-bench-serve | certificate(benchmark_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.yaml) bench_tool |
| `benchmark_mode` | lite | certificate(benchmark_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.yaml) benchmark_mode |
| `decode_tps_conc1` | 미기재 | lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`) |
| `floor_tps` | 미기재 | lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`) |
| `lite.cold_ttft_ms` | 369.0 | bench_report(bench_report_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.md) `## lite 지표` 표 master 열(단위 벗김 · 파싱만) |
| `lite.gen_tps` | 44.74 | bench_report(bench_report_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.md) `## lite 지표` 표 master 열(단위 벗김 · 파싱만) |
| `lite_cold_ttft_ms` | 369.1 | certificate(benchmark_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.yaml) lite_cold_ttft_ms |
| `lite_gen_src` | median_tpot | certificate(benchmark_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.yaml) lite_gen_src |
| `lite_gen_tps_warm` | 44.74 | certificate(benchmark_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.yaml) lite_gen_tps_warm |
| `lite_included` | true | certificate(benchmark_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.yaml) lite_included |
| `lite_kv_gib` | 8.0 | certificate(benchmark_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.yaml) lite_kv_gib |
| `ratio_M_over_primary` | 미기재 | lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`) |
| `rubric_authority` | 미기재 | lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`) |
| `verdict` | OBSERVATION-ONLY | lite-only 셀 — verdict_rule 에 투입되지 않았다(경량 리포트 `inform-only` · 판정 원천 없음 · benchmark_mode=lite · report_kind=lite) |
| `source` | certificate(benchmark_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.yaml) + bench_report(bench_report_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.md · lite) | — |

> **OBSERVATION-ONLY** — 이 태그의 성능 수치는 **관측 게재**다(인증서 없음). baseline·권고로 읽지 마라 — baseline 을 주장하려면 full_benchmark 인증서가 필요하다.
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## lite 지표 — 서빙 성공 직후 스냅샷 (결정론 파싱 · 손저작 ✗)

> 서빙 성공 직후의 **lite 스냅샷**이다 — cold 1회 + warm burst 1회. 동시성 곡선·반복·판정·인증서가 없다.
> 성능 baseline 이 아니며(OBSERVATION-ONLY) 수신자는 자기 환경에서 재측정한다.
> 출처: `bench_report_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.md` (bench_report(lite)) · 표는 리포트의 행을 그대로 옮긴다(재산정 ✗ · `--verify` diff 0).

| 메트릭 | Main | Sub |
|---|---|---|
| gen tokens/sec (warm) [master] | 44.74 t/s | — |
| cold-start TTFT [master] | 369 ms | — |
| GPU VRAM 점유 | ~82.6 GiB (serve-log 분해; nvidia-smi N/A) | N/A (통합메모리 nvidia-smi 미보고 · serve-log 분해 없음) |
| KV cache 점유 | 8.0 GiB (클러스터) | ≈ ÷TP (클러스터 공유) |
| 시스템 RAM 점유 | 105.5 GiB (87%) | 102.2 GiB (84%) |

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
| `source` | bench_report(bench_report_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.md) |
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
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `dcb713a7436c` 2026-09-30T13:35:58Z | 192fb275aa57..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@dcb713a7436c` | `git show dcb713a7436c2edd47dd4c1a034a2dfc8f51ed3b:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |

- `lite_bench.sh` — 경량 리포트 셀(lite_bench.sh 가 리포트를 발행한다)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

이 수치는 `lite_bench.sh`(측정 시각 이전 마지막 커밋 `dcb713a7436c`)가 서빙 컨테이너 **안**에서 `vllm bench serve` 를 두 번 부른 lite 측정이다 — cold 1회(요청 1 · warmup 0)와 warm burst 1회(요청 3 · warmup 1). 실행된 명령 줄 원문(셸 기록)은 계보에 없다 — 01 §1.4 bench 단계의 명령은 bench JSON 필드에서 재구성한 것이고(실행 원문 아님), 아래는 도구 원문이며 두 레그 모두 같은 `_bench` 함수가 부른다(같은 도구 · 같은 인자 · num-prompts · warmup 만 다름).

> [원문] lite_bench.sh@dcb713a7436c §L166-171
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

> [원문] lite_bench.sh@dcb713a7436c §L184-188
> MEASURED_UTC="$(date -u +%FT%TZ)"
> echo "[lite_bench] cold(단일·warmup0) + warm burst(N=$BURST_N·conc1·warmup1) ..."
> COLD_RC=0; _bench "$COLD" 1 0 >>"$BLOG" 2>&1 || COLD_RC=$?
> [ "$COLD_RC" = 0 ] || { echo "[lite_bench] cold bench 실패(rc $COLD_RC) — 출력 끝:" >&2; tail -5 "$BLOG" >&2; rm -f "$COLD"; }
> WARM_RC=0; _bench "$WARM" "$BURST_N" 1 >>"$BLOG" 2>&1 || WARM_RC=$?

> [원문] bench_report_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0 §lite 지표
> > cold(warmup 0 단일요청 → cold TTFT) 1회 + warm burst(N=3 · conc=1 · warmup 1) 1회. 산정·표 렌더는 `lite_metrics.py`(결정론)이며 이 문서는 그 표를 옮기지 않고 그대로 싣는다. 결측은 `N/A`(대체값 ✗).

**조건 목록**
- 도구 · 버전: `vllm bench serve`(서빙 컨테이너 안의 vLLM CLI · 03 §3.3 `bench_tool_version` 미기재 — 도구가 버전을 자기보고하지 않는다) · 래퍼 `lite_bench.sh@dcb713a7436c`.
- 백엔드 · 엔드포인트: `openai-chat` · `/v1/chat/completions`(리포트 측정 환경 표 `backend` · `endpoint`).
- 입력 / 출력 길이: random 512 → 128 토큰 · `--random-range-ratio 0`(고정 길이) · `--ignore-eos`(도구 원문 §L169-170).
- 샘플링 temperature · seed: 도구 원문에 인자가 없다 — `vllm bench serve` 의 기본값이 쓰였다(값은 이 원문으로 정해지지 않는다 · 단정하지 않는다).
- 동시성 · 요청 수: 두 레그 모두 `--max-concurrency 1` · `--request-rate inf` · cold 요청 1 · warm 요청 `BURST_N=3`(리포트 `burst_n` 3).
- warmup: cold 0 · warm 1(도구 원문 §L186 · §L188).
- 반복: 1(03 §3.3 `repeats` · `repeats_completed` — 반복 축 없음).
- warm/cold: gen t/s 44.74 = warm burst 의 median TPOT 에서 산정(인증서 `lite_gen_src` median_tpot) · cold-start TTFT 369 ms(리포트) / 369.1 ms(인증서) = cold 단일 요청.
- spec decode: 켜짐 — DSpark k=3 greedy(서빙 yaml · V20). 이 lite 측정의 acceptance length 는 warm 레그 2.0535 · cold 1.9104(03 §3.1 `accept_len` — lite_warm · lite_cold JSON `spec_decode_acceptance_length` · 판정 입력 아님)다. 같은 기동의 다른 측정값(L.A.I.L median 2.048 · Full 리포트 2.2011…)은 다른 측정의 것이다.
- 측정 노드: cluster(인증서 `measured_node` — 쌍이 하나의 측정 정체성) · 클라이언트는 master 컨테이너 안.
- 측정 시각(UTC): 2026-10-05T07:28:55Z(부하 직전 `MEASURED_UTC` · 03 §3.1).
- 서버 상태: 측정 기동(시도 3)은 READY 뒤 약 6시간 유휴가 있었다(02 §2.2 · testlog_26100420 §2.6). 이 lite 직전의 부하 이력은 계보에 기록이 없다 — cold TTFT 가 유휴 직후 첫 요청의 값인지는 단정하지 않는다.

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 1 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | vllm-bench-serve | bench_report(bench_report_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.md) |
| 현행 full 정의 충족 | 아니오 | 측정 구성 bench_mode=lite — full 측정이 아니다 · 판정점 반복 1 < 현행 정의 3 · 측정 도구 vllm-bench-serve — 현행 정의의 full 레그(GuideLLM)가 아니다 |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

이 측정의 강한 일치 키(인증서 `benchmark_26100516` — model `deepseek-v4.1-flash-exl3-2.0bpw-sfxnz` · gpu_model NVIDIA GB10 · vllm_version 0.30.0 · quantization exl3 · topology multi · tensor_parallel_size 2)와 소프트 지문(image_tag `easy-vllm:0.30.0-cu133-aarch64-source-sm12x` · max_model_len 1048576 · max_num_seqs 2 · kv_cache_memory_bytes 8589934592 · gpu_memory_utilization 0.80 · bench_tool vllm-bench-serve · image_digest · kv_cache_dtype · moe_backend · attention_backend · ple_mode 는 N/A)를 기준으로 본다.

**같은 셀의 다른 측정 — Full 리포트(bench_report_26100516_41_31 · 생성 07:41:31Z)의 lite 레그.** 같은 기동(시도 3) · 같은 이미지 태그 · 같은 서빙 설정이고, 리포트는 "full 런이 lite 를 포함해서 실행한 결과" 라고 적는다. 이 lite 측정보다 **뒤**다. lite 는 레벨이 하나(conc 1)뿐이라 표는 그 한 레벨의 두 지표다(값은 두 원천의 글자 그대로 · 비 = 이 측정 ÷ 그 레그 · 소수 셋째 자리 반올림).

| 지표 | 이 측정 | 같은 셀 Full 의 lite 레그 | 비 |
|---|---|---|---|
| gen tokens/sec (warm) [master] | 44.74 t/s | 42.85 t/s | 1.044 |
| cold-start TTFT [master] | 369 ms | 373 ms | 0.989 |
| acceptance length(warm) | 2.0535 | 2.20 | 0.933 |

warm gen t/s 차이(+4.4%)는 계획의 노이즈 기준(단일 burst 차이 3% 는 순위가 아니다 — plan_26100319 §5)보다 크다 — "같은 범위 재현" 이라 부르지 않는다. 두 측정 모두 spec 이 켜져 있어 비교는 성립하고, 수락 길이 비(0.933 — 이 측정 2.0535 · lite 레그 2.20, 각 레그의 bench 결과 · 소수 셋째 자리)는 처리량 비(1.044)와 **반대 방향**이다 — 수락이 더 낮은 쪽이 더 빨랐으므로 이 차이는 수락으로 **설명되지 않는다**(단일 burst · 반복 없음 · Q8). 이 비교의 소프트 지문 차이: Full 리포트 스냅샷은 kv_cache_dtype fp8 · moe_backend deepgemm_mxfp4 · enforce_eager false · driver_version 580.173.02 · cuda_version 132 를 싣고 이 인증서는 그 키들이 N/A 이거나 없다 — 같은 기동이므로 값이 실제로 다른 것이 아니라 **기록 범위가 다른 것**이다(Full 스냅샷의 driver 는 선언값 — M5 · moe_backend 키는 서빙 yaml 에 없고 엔진 로그는 EXL3 native p2b 경로를 보인다 — 그 값의 출처는 확인하지 못했다).

- 같은 기동(기동 3) Full GuideLLM(c=1 39.68 / 40.64 / 39.94 · c=2 31.41 / 30.72 decode t/s · 1024/256 · 레벨당 16 요청) · 도구가 다름(guidellm 0.7.3 대 vllm-bench-serve) · 입출력 길이 다름(1024/256 대 512/128) · **비교 불가** — 부하 조건이 다르다. 게다가 이 스윕은 c=2 run3 끝의 세션 열 가드 kill 로 **Full 근거에서 제외**됐다(testlog_26100510 §4).
- 기동 4 Full c=1(34.41 cold · 40.83 · 39.80 t/s · GuideLLM 1024/256 × 3 · 열 kill 없음 · bench_mode full · weak PASS · 리포트 bench_report_26100613 — 계보 목록에는 testlog_26100510 §4.5 를 통해서만 실림) · **다른 기동**(측정 뒤 재기동 · 2026-10-06) · 도구 · 부하가 이 lite 와 다름 · **비교 불가**(이 lite 와) — 이 셀의 유일한 full 등급 측정이며 수치는 그 testlog 발췌로만 인용한다(02 §2.2 W10).
- 기동 1 lite(41.53 t/s · 수락 2.18 · 2026-10-05T00:36:59Z · bench_report_26100509) · 같은 도구 · 같은 이미지 · config 의 **다른 기동** · 비교 가능(형상 비교 — 다른 기동 프로세스) · 비 1.077 · 수락 비 0.942(2.0535 ÷ 2.18) — 역시 수락이 낮은 쪽이 빨라 수락으로 설명되지 않는다(testlog_26100510 §4).
- K5 진단 셀 Full 의 lite 레그 · 45.56 t/s · cold 370 ms · 이미지 태그 `…-sm12x-v10probe`(POST 74 없음 · `DSV41_MOE_PREP_FUSED` 없음) · **비교 가능(형상 비교 · stale)** — 다른 소프트 지문: image_tag(태그만 비교 가능 — 인증서 쪽 digest N/A) · 그 리포트는 kv_cache_dtype · moe_backend · driver · cuda 를 싣고 이 인증서는 N/A — 비 0.982(bench_report_26100418_19_37 lite 표).
- K6 진단 셀 Full 의 lite 레그 · 40.71 t/s · 이미지 태그 `…-sm12x-v11probe`(이 셀과 같은 레버 env · 진단 이미지) · **비교 가능(형상 비교 · stale)** — image_tag 가 다르고 나머지 소프트 지문 차이는 K5 항목과 같은 기록 범위 차이 — 비 1.099(bench_report_26100421 lite 표). 같은 레버가 진단 이미지에서 40.71 · 정식 이미지에서 44.74 로 나온 차이는 설명되지 않았다(Q8).
- F2 정식 재빌드(K5 구성 · POST 74 없음) Full 의 lite 레그 · 38.98 t/s · 이미지 태그가 **같은 문자열**이지만 F2 는 74 없는 이전 빌드다(태그는 가변 포인터 — testlog_26100420 §1) · **비교 가능(형상 비교 · stale)** — 태그가 같아 소프트 지문만으로는 갈리지 않는다 · 내용(패치 74)과 셀 env(`DSV41_MOE_PREP_FUSED`)가 다르다 — 비 1.148(bench_report_26100420_31_35 lite 표).
- K4 진단 셀 Full 의 lite 레그 · 33.80 t/s · 이미지 `…-sm12x-v9probe`(dense_gemv · moe_prep 없음) · **비교 가능(형상 비교 · stale)** — 비 1.324(bench_report_26100417_38_45 lite 표).
- 10-03 eager 셀탐색 lite(18.78 · 19.35 · 13.90 · 13.89 t/s · cold TTFT 2.2~2.5 s) · 같은 도구 · 같은 강한 키 · 그 리포트들의 측정 환경 스냅샷은 소프트 지문 키를 싣지 않는다(image_tag · max_len · KV 등 미기재) · plan 은 그 셀들이 eager 이고 spec k 가 0 · 3 · 5 · 10 이었다고 적는다 · **비교 가능(형상 비교 · stale)** 로 읽되 소프트 지문을 대조할 수 없어 차이를 하나로 설명하지 않는다(bench_report_26100314 · 26100318 · 26100318_44_52 · 26100319).
- 참조 런타임 R-1(저자 0909 베이스 · lite 39.74 · L.A.I.L 44.50 — devlog_26100610 §1 · §3) · 강한 키 `vllm_version` 이 다르다(저자 변종 빌드) · **무효**(인증서 규칙) — 격차의 방향을 보는 참고로만 쓴다.
- 외부 보고(sfxnz prose c=1 50.41 tok/s · coolbho3k 1M 프롬프트 32.5 t/s) · 측정 도구 · 부하(짧은 프롬프트 · ignore_eos · 1M prefill)가 다르고 우리 측정이 아니다 · **비교 불가**(plan_26100208 §8 D7 · plan_26100319 §1).

**손 승계 입력.** 이 lite 측정은 verdict_rule 에 투입되지 않아 판정 입력(expected · floor · ratio · accept_len)이 없다. 같은 셀 Full 리포트의 weak PASS 는 accept_len 2.2011…(source measured)을 썼지만 루프라인 상한이 측정의 15분의 1 이라 그 판정은 정보가 없다(M4).

**판정 등급.** 권위 = **관측 게재(OBSERVATION-ONLY)** — lite 등급 인증서(`benchmark_mode: lite` · `verdict: not_applicable` · `lite_verdict: pass`)는 hint 발행 자격의 근거이지 성능 판정이 아니다. 현행 full 정의(docs.md:42 — lite 게이트 → GuideLLM × 반복 ≥3)로 이 측정의 충족은 위 사실 블록대로 **아니오**다(bench_mode lite · 반복 1 · 도구 vllm-bench-serve) — baseline 이 아니다. 같은 셀의 full 등급 측정은 기동 4 의 Full c=1 뿐이고(다른 기동 · weak 권한이라 PASS 여도 인증서 없음 · 루프라인 결함 M4), 기동 2 · 3 의 Full 스윕은 열 가드 kill 로 근거에서 빠졌다. 재측정이 필요한 조건: 네 HW 에서는 항상 · 이 HW 에서도 이미지 재빌드 · 커널 레버 env 변경 · 수락 길이가 다른 프롬프트 세트일 때.
