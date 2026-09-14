# lite 모드 + full-모드 종결 발행 (조건부 reference)

> 두 개의 **서로 다른 모드**를 담는다: ① lite = serve 성공 직후 자동으로 도는 inform-only 스냅샷,
> ② full 종결 발행 = 적대 게이트가 끝난 뒤의 report/인증서. 둘 다 **done-게이트가 아니다**
> (done-게이트는 `verdict_rule.py` 독점).

## 1. 경량(lite) 모드 — inform-only · 기본 ON (plan_26071115 · Phase B)

> full 적대 게이트(loop-until-done)와 **다른 모드**. lite 는 **서빙이 성공하면 자동으로 도는 가벼운 상태-스냅샷**이다 —
> 배포 사용자가 "일단 떴다" 다음 곧바로 *속도·용량 현재치*를 눈으로 확인하게 해 검증 투명성을 올린다.
> **소유 = adversarial-benchmark**(explorer 는 트리거·핸드오프만).

- **기본 ON**: serve 성공 직후 **자동 수행**. 가벼운 스킵 시그널("스킵해"·묵시적 넘어감)에도 **수행한다** — 경량 벤치는
  시스템 안정성과 직결되어 *개발자 의지*로 기본 실행이다. **억제 = 강력 거부 구문**("무조건 어떠한 경우에서라도 구동하지 마" 급)만,
  그리고 **세션 한정**(config/manifest 영구 기록 ✗; 매 세션 기본 ON 복귀). 영구 opt-out 불허(안정성=프로젝트 신뢰성 직결).
- **inform-only**: **PASS/FAIL 판정 없음 · 자동 loop-back 없음**. `verdict_rule.py` 에 투입하지 않는다(done-게이트는 오직
  full 경로 소유 — 기능≠성능 따름정리 불변). lite 출력이 커뮤니티/레퍼런스 대비 **심각한 괴리**로 보이면 →
  **이상징후 안내 + full 승격 권유**까지만(자동 재탐색 ✗).
- **괴리 판단 = 에이전트 재량**: `references.md` §4 HW-스코프 baseline·포럼 수치 대비 *정성 판단*.
  **정량 threshold 금지**(커뮤니티 자료 신뢰성은 가변). 결정론 게이트화 ✗.
- **측정 사양**: `lite_bench.sh <config>` — cold(무-warmup 단일요청→cold TTFT 별도 1줄) + warm burst(**N=3**·conc=1·warmup 1 제외,
  `config bench.lite_burst_n`/`--burst-n` 조정) 2회 `vllm bench serve`. **5종 메트릭**: gen tokens/sec(warm, =1000/median_tpot) ·
  cold-start TTFT · GPU VRAM 점유(GiB+%) · KV cache 점유(GiB+%) · 시스템 RAM(GiB+%). 산정·표 렌더 = 결정론 `lite_metrics.py`.
  통합메모리(GB10)는 nvidia-smi 메모리 N/A → **serve-log VRAM 분해 폴백**, 그도 없으면 값 없이 "N/A(source)" 음성정직
  (대체값 날조 ✗). engine-log KV 라인 부재 시 fail-soft null.
- **출력**: **single = 채팅 5행 표**(메트릭|값 · 용량은 GiB+% 병기 · cold TTFT 별도 행). **multi = 병합 표 1개** —
  용량 3종 열=**Main|Sub**(per-node), 속도·cold TTFT 는 마스터 엔드포인트 기준 1행(신분차는 명령체계뿐, 관측은 평등).
- **멀티 수집 (A2A 정합)**: per-node 평등 수집. 서브 = **SSH 읽기전용 probe**(`nvidia-smi`/`/proc/meminfo` —
  `multinode_serve_smoke.sh` 패턴 재사용). 이 읽기전용 런타임 관측은 **health 폴링과 동형 평면**이지 "서브 작업코드/설정
  재스캔 금지"(A2A 경계)와 **다른 평면**이다. 서브 probe 실패 시 graceful — 마스터 단독 + 실패 음성정직 표기.
- **down 시나리오**: "테스트만 하고 down" 케이스도 serve → **lite 수행** → 결과 표시 → down(라이브 엔드포인트가 있는 동안 측정).
  용처 매뉴얼은 생략(recipe-explorer 소관).
- **경량 리포트 발행(`--publish-report` · 2026-09-14 · `plan_26091407` §4.5 · 사용자 결정 Q4·Q10)**: lite 만 잰 셀도 hint 를
  낸다. 그 hint(`hint_map_only`)가 바인딩할 문서를 `lite_bench.sh <config> … --publish-report` 의 종결부가
  `render_report.py --lite-only --lite-raw-json <raw>` 로 발행한다 — `docs/benchmark/bench_report_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md`
  (full 리포트와 **같은 접두사·같은 명명 SSOT** · publisher 정규식·work-manifest 스키마 무변경). 본문 = 헤더 `mode: lite` ·
  **측정 구성 표**(`bench_mode=lite` · `bench_mode_kind=declared-lite`(기록 모양은 `classify_cell.declared_lite_record`) · 도구
  `vllm-bench-serve` · 반복 1 · `downgrade_reason` 없음) · lite 지표 5종 표(lite_metrics 표 그대로) · 환경 스냅샷. 판정·루프라인·
  동시성 곡선·인증서는 **없다**. 명명 키(model·gpu·vllm)는 raw 가 남긴 config yaml·envfile·manifest·엔진 로그 경로에서
  `sweep_bench` 조립부와 같은 규칙으로 파생하고(`render_report.lite_identity` · 두 자리는 `scripts/selftest_lite_report.py` 가
  교차검증), 측정시각은 raw 의 `measured_utc`(부하 직전 호스트 UTC)다 — 없으면 exit 2(날조 ✗).
  - **기본은 발행하지 않는다**(자동 핸드오프 경로에 부작용 ✗): ① 서빙 직후 자동 lite 는 **관측·inform-only 한정** 예외다.
    ② `sweep_bench.sh` 가 이 스크립트를 lite 레그로 부른다 — 레그가 리포트를 내면 같은 시간대·같은 조합의 full 리포트가
    `_MM_SS` 로 밀려 인증서와 stem 이 갈라지고 `publish-benchmark` 가 `…_STEM_MISMATCH` 로 거부한다(full ⊇ lite 이므로 full
    리포트가 lite 표를 이미 품는다). ③ 발행 실패(명명 키 불성립·충돌)는 요청한 호출에서만 **exit 5** 이고 raw·warm·cold·엔진
    로그는 남는다(재측정 없이 `render_report.py --lite-only` 로 재렌더).
  - ⚠ 같은 모델·GPU·버전의 **full 스윕과 같은 시간대(KST 시)** 에 `--publish-report` 를 돌리면 ②와 같은 이름 밀림이 생긴다.
    lite-only 로 선언한 셀에서만 쓰고, 같은 조합의 full 셀과 같은 KST 시에 두지 않는다. 가드는 아직 없다 — 이름 밀림은
    lite 고유 결함이 아니라 `doc_naming` 이 report·인증서를 **종류별로 따로** 스캔하는 구조의 결함이다(같은 시간대 full 스윕
    둘 중 앞 스윕이 인증서 없이 끝나도 같다). 교정 자리는 명명 SSOT(후속).
  - 바인딩: `evidence_publisher.py publish-lite-report --topic <map_only 토픽> --bench-report-src <그 리포트>`(복사 ✗ · 측정 구성
    표가 `bench_mode | lite` 라고 말하는 리포트만 받는다). 결손 코드·카탈로그 컬럼은 hint-publisher 소관(계약 §3.0.1).
- **자동 핸드오프 = 헌법 명시 예외**: recipe→adversarial **lite 한정** 자동 수행은 "무인 자동실행 없음" 트리거 정책의
  **명시 예외**다(안전망 데몬 예외와 동형 — 관측·inform-only 한정). **full 벤치·bump·다운로드의 완전-수동 속성은 불변**.
  Flag 게이트: lite 는 이미 Flag-게이트된 serve 위에서 돈다(전이적) + `lite_bench.sh` 가 `run_bench.sh` 와 동형
  **fail-closed 백스톱**(키·MC·Flag 부재 exit 4).

## 2. full-모드 종결 발행 — report + 인증서 (편지 패턴 A·B · plan_26071510)

> lite 와 다른 **full 경로의 종결 산출물**. full 적대 게이트가 **종결**(cap 소진 or PASS)되면, 판정과 **별개로**
> 사람용 report + (PASS시)기계용 인증서를 `docs/benchmark/`(문서형 · docs.md §benchmark)에 발행한다.

- **부하 스윕(client-load · reload 0)** — `sweep_bench.sh <config> [--topology] [--levels 1,2,4,8,16]`: 단일 running serve 에
  **동시성만** 변화(reload 0 — 벤치마커 "기동 안 함" 불변식 보존). **판정점(동시성=1) 강제 포함** → verdict 재사용(재측정 0).
  **적응 상한 클램프 + 절삭 로그**(레벨 실패 시 상위 중단·"레벨 N 절삭" 기록 — silent truncation ✗). 각 레벨 = `run_bench.sh`
  메커니즘 재사용(Flag/A2A 게이트 전이). config-space(batch×maxlen) reload 는 이 스윕 **밖**(Max/explorer 소관).
- **full 의 정의 = `lite ∪ GuideLLM × 반복 ≥3`**(2026-09-14 · `plan_26091407` §4.4 · 사용자 결정 Q3) — 레벨마다 같은
  serve 에 **반복 ≥3**(`repeat_kind=warm-rerun` · `cold-restart` 는 선언 슬롯만). 반복은 분산·신뢰성의 최소조건이다:
  1회 측정에는 산포 추정치가 없고, 다른 도구·조건의 밴드를 빌리면 추론이 틀린다(`audit_26091323` §2.4b).
  - **반복 수**: `sweep_bench.sh --repeats N` > 활성 캠페인 `campaign.yaml budgets.repeats` > full 정의값 3
    (해소·하한의 소유 `repeat_axis.py` · 출처는 `sweep_index.repetition.requested_source`). **N<3 은 exit 2** —
    반복을 낮춰 full 을 선언하지 않는다(lite 만 재려면 §1 lite 통로). Broad Search 에서는 `init` 이 이 값을
    `declared_budget.repeats` 로 예산에 싣고 `cell` 이 넘긴다(셀 비용 = 레벨 × 반복 — 벽시계 예산의 근거).
  - **반복 대상은 레벨 측정 레그**다. lite 선행 레그는 1회(cold TTFT 는 반복하면 cold 가 아니다)이고, 레벨 run
    들이 그 lite warm JSON 을 spec 승계원으로 함께 쓴다.
  - **raw**: run 1 = `level_NN/`(대표 run · 종전 배치), run k≥2 = `level_NN/run_KK/`. 대표 `measured.json` 은 파서
    필드를 그대로 두고 `runs[]`·`repeats_completed`·`repro_band_pct`·`repro_band_source=measured(n=N)`·
    `repeat_kind` 를 덧붙인다 — judge_bench 의 accept_len 승계·verdict·인증서는 **대표 run 1회**를 읽는다(평균·합성 ✗).
    재현 밴드 = `(max−min)/mean×100`(완주 run 의 decode_tps · 2회 미만이면 N/A)이며 **기재**다.
  - **스윕이 멈춘 자리(즉시 신호 · `sweep_index.repetition.stop`)**: 레벨 첫 run 실패는 종전 적응 상한 클램프(절삭 ·
    `stop.kind=clamp` · 그 레벨은 index levels 에 없으므로 경계 사실은 `repetition.clamp_run`)다. run k≥2 실패
    (measurement_ok=false · run_bench 비0)는 **반복 중단**(`stop.kind=repeat-break`)으로 남은 반복·상위 레벨을
    멈춘다 — 끊긴 동시성은 반복해 버티지 못한 포화 경계라 상위도 무너진다(클램프와 같은 이유 · 요청 N>3 에서
    완주가 이미 ≥3 인 레벨이어도 같다). 시각 대조 창 = 직전 run 시작~끊긴 run 끝(클램프는 아래 측정 레벨의 마지막 run 시작~
    클램프 run 끝).
  - **강등(기계 이벤트만 · 확정 `classify_cell.py` post-hoc)** — 반복 조건의 범위는 **판정점**(동시성 1 · 인증서·판정이
    묶이는 레벨)이다(2026-09-14 리뷰 정정: 종전의 "측정 레벨 전체 완주 min" 은 같은 포화 경계를 첫 run 실패면 full·
    둘째 run 실패면 lite 로 반대로 판정했다):
    ① 멈춘 자리의 창 안에 **집행된** 블랙박스 사살(KILL_EVENT_KINDS · 허용오차 0)이 있으면 레벨·run 순번과 무관하게
       `bench_mode=lite` · `downgrade_reason=blackbox_kill`(사용자 결정 — kill 이벤트는 기계 이벤트 트리거다 · 가장 흔한
       실사살 형태인 높은 동시성 레벨의 첫 run 사살을 절삭으로 삼키지 않는다)
    ② 그 밖에 판정점 완주 ≥3 → `full`(경계 레벨의 반복 중단·첫 run 실패는 클램프 · 기재)
    ③ 판정점에서 끊겨 완주 <3 → `lite` · `run_failed`(트립 단독·시각 불일치·이벤트 미관측 포함 — 관측된 신호 그대로 · 추측 ✗)
    ④ runs[] 부재 레벨(옛 산출물·집계 실패)·판정점 부재·끊김 없는 정의 미만·경계 밖 정의 미만 → 판정하지 않는다(null)
    대조 여부는 구조 필드 `downgrade_correlation` ∈ {`matched`, `miss`, `not_scanned`, `unavailable`, `not_applicable`}.
    **분산(밴드 폭)은 강등 사유가 아니다.**
  - **판정 기록을 쓰는 손**: `sweep_bench.sh` 종료부가 `classify_cell.py --sweep-index … --events-from-repo <repo>
    --write-bench-mode` 를 부른다(측정·재조립 둘 다) — 정규 경로(sweep_bench → judge_bench → render_report·인증서)와
    Broad Search 가 **같은 기록 하나**를 읽는다. 기록은 원자적으로 쓰이고 `sweep_index_generated_utc` 로 측정에 묶인다.
  - **읽는 자리**(단계 ⑤ lite hint 통로가 결정론으로 읽는다): 스윕 디렉터리의 `bench_mode.json`(정본 · 판독은
    `classify_cell.read_bench_mode_record` → `ok`·`absent`·`unreadable`·`stale`)과 Broad Search 셀 기록(그 정본의 사본:
    `bench_mode`·`bench_mode_source`·`downgrade_reason`·`downgrade_reason_source`·`downgrade_correlation` + sweep 요약
    `repetition`). **강등된 lite** = 사유 값 · **선언된 lite-only** = 사유 null ∧ `bench_mode_source` 가 `declared(` 로
    시작(판독 규칙 `classify_cell.bench_mode_kind`).
  - **반복 수 출처의 자리별 키**(같은 개념 · 각 문서의 이름공간을 따른다 — 단계 ⑤ 측정 구성 표가 읽을 때 대응표):

    | 자리 | 반복 수 | 출처 |
    |---|---|---|
    | `repeat_axis.py resolve` 출력(sweep_bench 셸 변수) | `REPEATS` | `REPEATS_SOURCE` |
    | `sweep_index.json` `repetition` | `requested` | `requested_source` |
    | 대표 `level_NN/measured.json` | `repeats_requested` | `repeats_requested_source` |
    | Broad Search 상태 `declared_budget` | `repeats` | `repeats_source` |
    | `sweep_stop.py` 정지 판정 | `declared_budget.repeats` | `budget_repeats_source`(미선언이면 `absent(…)`) |

  - ⚠ 정상 E2E(판정점 반복 완주 · 사살 없음 — 포화 경계에서 스윕이 멈추는 클램프 포함)는 강등 경로를 밟지 않는다 —
    그 경로는 `scripts/selftest_sweep_repeats.py` 실패주입이 지킨다.
- **사람용 report(항상)** — `render_report.py --sweep-index <sweep_index.json> --verdict-json <verdict> [--roofline-json]`
  → `docs/benchmark/bench_report_<YYMMDDHH>_<model>_<gpu>_<vllm>.md`. **PASS/FAIL 무관 발행**("왜 느렸나"도 사람이 봐야).
  **inform-only**(verdict 를 *표시만* — 판정권한 ✗·verdict_rule 독점) · 결정론 렌더(LLM 표·숫자 저작 ✗) · N/A fail-soft.
  bench_mode 판정 기록의 부재·판독 실패·다른 측정의 기록은 **발행을 막지 않고** "미확정 — 사유" 로 적는다(명시
  `--bench-mode-json` 이 그러면 exit 2). 판정 절 앞에 **측정 구성 표**(`bench_mode`·`bench_mode_kind`·출처·`downgrade_reason`·
  도구·버전·요청 반복·판정점 완주)를 싣는다 — 경량 리포트와 같은 제목·키이고(`render_report.MEASUREMENT_CONFIG_*` =
  hint 파서 `render_bench_section.MEASUREMENT_CONFIG_*`), 강등 셀의 리포트는 이 표가 `bench_mode | lite` 라고 말하므로
  같은 lite 통로의 바인딩 대상이 된다(`evidence_publisher init --downgrade-from full_benchmark --downgrade-reason <사유>`
  가 full_benchmark 토픽을 map_only 로 재분류하며 바인딩을 보존한다 · 사유 어휘 = `classify_cell.DOWNGRADE_REASONS` ·
  재분류는 바인딩된 리포트의 이 표가 `downgraded-lite` 와 같은 사유를 말할 때만 열린다 — 선언만으로는 열리지 않는다).
  표의 `*_source` 칸에는 블랙박스 events 파일 경로가 들어갈 수 있다 — 리포트는 비배포 docs 평면이라 그대로 두고, hint
  발행기는 이 칸을 배포 페이로드로 옮기지 않는다.
- **기계용 인증서(PASS시만)** — `publish_benchmark_record.py --sweep-index … --verdict-json …`
  → `docs/benchmark/benchmark_<YYMMDDHH>_<model>_<gpu>_<vllm>.yaml`. **flat 계약**(중첩 ✗ — 소비자 stdlib 독해) +
  **carry-forward 재검증 헤더**(강한키=model/gpu/vllm/quant/topology/tp 정확일치 + 소프트지문=driver/cuda/image/max-len/
  kv-bytes/gmu/moe 불일치 시 stale). verdict≠PASS 면 **미발행**(report 만). 반복 축 산출물(`repetition.requested`
  가 정수)이면 bench_mode 판정 기록이 **full 일 때만** 발행한다 — 강등(lite)·기록 부재·낡음·판정 불가(집계 실패 등)는
  PASS 여도 `benchmark_mode: full` 인증서를 **내지 않는다**(2026-09-14 · 거짓 주장 ✗ · 강등 셀의 통로는 lite ·
  plan §4.5 `--downgrade-from full_benchmark` → map_only 는 인증서를 요구하지 않는다). 발행기는 완주 수를 다시 세지
  않고 판정 기록을 읽는다. 반복 축 이전 산출물(repetition 없음)은 종전대로다.
  인증서는 대표 run 1회 값과 스윕 측정시각 1개만 실어 반복이 인증서 키(강한 6키 + `measured_utc`)를 늘리지 않는다.
  **`rubric_authority: weak|explicit|explore`** 를 검증결과 블록에 함께 싣는다(2026-08-22 · `plan_26082219` A6) —
  승격 판정기(`completion_gate.py`)가 *어느 권한에서 잰 판정인지*를 인증서에서 직접 읽어야 explore 계약
  (성능 판정=서술 · 승격 게이트=서빙 성립+유효 측정)을 집행할 수 있다. 결측은 `N/A` fail-soft이며,
  **legacy 인증서(필드 부재)는 관용**한다(값역 검사만 — 스키마가 늘었다고 기존 판정을 뒤집지 않는다).
  같은 판정기가 `floor_tps > 0` ∧ `ratio_M_over_primary` 실수 ∧ `primary_source` 실재를 **fail-closed** 로
  요구한다 — 손저작·개조·stale 인증서의 공허 PASS 승격을 이중으로 막는다.
- **승격 carrier(REFUTE 런 필수 · 2026-08-24 · `plan_26082405`)** — 인증서는 PASS 전용이므로 **REFUTE 런의
  루브릭 권한은 인증서로 게이트에 도달할 수 없다**. 그래서 증거 발행 시 판정기 산출물을 함께 넘긴다:
  `evidence_publisher.py publish-benchmark … --verdict-json-src <verdict.json>`.
  그러면 `rubric_authority`·`floor_tps`·`ratio_M_over_primary`·`primary_source` 가 출처 표시
  (`rubric_source: verdict_json`)와 함께 work-manifest 의 `benchmark` 오브젝트에 실리고,
  `completion_gate.py` 가 인증서 부재 시 이 채널을 fallback 으로 읽는다.
  **이 플래그를 빠뜨리면 explore-REFUTE 는 종전대로 `BENCHMARK_VERDICT_NOT_PASS` 로 막히고
  사람 `perf_waiver` 서명을 강요한다** — 값이 없어서가 아니라 통로를 안 열어서다(2026-08-24 실측 결함:
  bench report 는 "루브릭 권한 = explore" 를 적고 있는데 게이트는 못 봤다).
  계약은 인증서와 **동일**하다(`floor>0` ∧ `ratio` 유한 ∧ `primary_source` 실재) — 완화가 아니라 carrier
  교체이며, `weak`/`explicit` REFUTE 는 여전히 `perf_waiver` 없이는 열리지 않는다.
- **비용 규율**: 재탐색 루프 **내부는 값싼 단일점 판정** 유지 · 스윕·리치리포트는 **종결 1회**만. 오케스트레이션은
  **에이전트 매개**(스킬↔스킬 직접호출 ✗). **done-게이트는 여전히 verdict 독점** · lite 는 기본적으로 채팅 표만이고
  문서는 lite-only 셀이 `--publish-report` 로 명시할 때만 낸다(§1 · 판정·인증서는 어느 경우에도 없다).
- 발행 경로·명명 SSOT = `.claude/skills/wiki-desk/scripts/doc_naming.py`(generated_utc→KST) · 증거 계약은 `.claude/policies/runtime/completion_gate.py` 가 판정.
