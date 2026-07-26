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
- **사람용 report(항상)** — `render_report.py --sweep-index <sweep_index.json> --verdict-json <verdict> [--roofline-json]`
  → `docs/benchmark/bench_report_<YYMMDDHH>_<model>_<gpu>_<vllm>.md`. **PASS/FAIL 무관 발행**("왜 느렸나"도 사람이 봐야).
  **inform-only**(verdict 를 *표시만* — 판정권한 ✗·verdict_rule 독점) · 결정론 렌더(LLM 표·숫자 저작 ✗) · N/A fail-soft.
- **기계용 인증서(PASS시만)** — `publish_benchmark_record.py --sweep-index … --verdict-json …`
  → `docs/benchmark/benchmark_<YYMMDDHH>_<model>_<gpu>_<vllm>.yaml`. **flat 계약**(중첩 ✗ — 소비자 stdlib 독해) +
  **carry-forward 재검증 헤더**(강한키=model/gpu/vllm/quant/topology/tp 정확일치 + 소프트지문=driver/cuda/image/max-len/
  kv-bytes/gmu/moe 불일치 시 stale). verdict≠PASS 면 **미발행**(report 만).
- **비용 규율**: 재탐색 루프 **내부는 값싼 단일점 판정** 유지 · 스윕·리치리포트는 **종결 1회**만. 오케스트레이션은
  **에이전트 매개**(스킬↔스킬 직접호출 ✗). **done-게이트는 여전히 verdict 독점** · lite 는 발행 안 함(채팅 표만).
- 발행 경로·명명 SSOT = `scripts/doc_naming.py`(generated_utc→KST) · 증거 계약은 `scripts/completion_gate.py` 가 판정.
