# hint 태그 발행 계약 (v7 · 2026-09-29 — 결정론 3축 + 근거 붙은 꼬리 · native 구조 통일 · 판정 표면 · 안내 커밋 부모)

> 정본. 집행자는 단일 진입 `.claude/skills/hint-publisher/scripts/hint.py` 와 그 패키지 `scripts/hintlib/`
> (naming · evidence · lineage · artifacts · template · pii · branch · tag · catalog)다. 이 문서는 **규칙과 그 이유**를 적는다 —
> 명령별 인자는 `hint.py <명령> --help`, 운영 요약은 스킬 `.claude/skills/hint-publisher/SKILL.md` 가 든다(같은 규칙을 두 자리에
> 다시 적지 않는다). 설계 근거 `docs/plan/plan_26092908`(v7 · §3 결정 U1–U9 · §4 계약) ← `docs/plan/plan_26092119`(v6 · §3 결정
> P1–P3·D1–D12 · §4 설계 · Execution approval O2–O6). **새 동작의 정본은 코드다** — 이 문서가 코드와 어긋나면 문서를 고친다.
> **v7 은 발효(2026-09-29) 이후 발행분에만 적용한다** — 원격에 올라간 과거 태그(v6 이하)는 판정·교정·리콜하지 않는다(§9 · P1).

## 1. hint 태그는 무엇인가

**이 프로젝트를 배포받은 다른 코드에이전트가 토큰을 아끼도록 "정답지의 일부를 살짝 보여주는" 자료다.**

- 목적은 **토큰 이코노미**다. 완제품 레시피가 아니라 **여정의 지도**다 — 에이전트 작업의 입력 토큰은 대부분 *탐색*이 먹으므로,
  이미 뚫린 벽의 순서를 넘겨주는 것이 가장 큰 절약이다.
- 정보가 적은 태그는 **결함이 아니다** — 받는 에이전트가 나머지 여정을 스스로 밟을 뿐이다. 정보가 많은 신판은 참조할 데이터가
  늘어난 것이지 구판을 무효화하지 않는다. 그래서 **규약 세대 차이는 태그의 유효성과 무관하다**(§9).
- **셀 1개 = 태그 1개**(v6 · D9). 발행 단위는 캠페인의 셀(버전×모델×레시피 조합을 한 노드 형상이 수행한 것) 하나다 —
  캠페인 단위 일괄 발행은 모든 셀의 사실이 한 페이로드에 섞여 저작도 검증도 더 어렵다.
- **한 태그 = 한 노드 형상의 수행 기록**(v4 · 2026-09-06 사용자 결정). "이 조합이 된다" 가 아니라 "**어느 노드 형상이 무엇을
  겪었나**" 가 태그의 내용이다. 멀티의 쌍은 하나의 수행 정체성이므로 `cluster` 로 한 번 발행한다. 그 형상이 이름의 arch
  세그먼트다(§4).
- 태그의 **zip(archive) 하나가 곧 지도 · 서사 · 재현 키트**다(§7). annotated 태그 본문은 요약 한 문단 · 포인터 · 증거 주소
  footer 뿐이다(D4 — 옛 형식은 재현에 결정적인 핀을 zip 에 들어가지 않는 태그 본문에만 두었다 · plan §2.2 F1).
- **Docker 셀과 native 셀은 같은 슬롯 구조**다(v7 · U1). native 에서 실행 검증할 수 없는 파일도 생성해 싣고 **파일 단위**로
  `generated-unverified` 라 표시한다(§3.3) — "native 는 zip 만으로 재현되지 않는다"(v6 적대 검증 V6)를 구조로 닫는다.

## 2. 진짜 위협 — 차단해야 할 단 하나

> **서빙이 실패했는데도 에이전트가 사용자를 속여 "서빙되었다"고 허위 기재한 정보가 배포되는 것.**

이것이 최대 위협이며 게이트의 존재 이유다. 형식 미비는 위협이 아니다. v1 게이트는 형식(footer 유무)으로 차단하고 증거(서빙
성공)는 검사하지 않았고, v2 가 차단 사유를 **형식 미비 → 증거 부재·위조**로 옮겼다.

**v6 — 증거원은 선언이 아니라 관측이다**(X8 · plan §2.2 F11: 옛 `runtime.health_ok` 는 호출자가 적은 선언이었고, 일괄 래퍼가
`pii_scan.passed: true` 까지 합성했다). `publish` 는 발행 자격을 아래 **관측 파일**에서만 판정한다(`evidence.qualification`).

| 순위 | 관측 | 조인(이름만으로 묶지 않는다) |
|---|---|---|
| ① | `output/<t>/benchlog/serve_proof_<cell>.json` — 멀티 스모크(`multinode_serve_smoke.sh`)가 serve 시점에 적는 health 200 + 추론 판정 | 셀 이름 · 이미지 |
| ② | 스윕 `level_*/post_health_<cell>.json`(health 200 ∧ running ∧ ¬OOM) ∧ 같은 레벨 `bench_<cell>.json` completed ≥ 1 | 스윕 `generated_utc` = 인증서 `measured_utc`(인증서가 없으면 리포트 생성일) |
| ③ | lite 셀: `lite_raw_<cell>.json`(config_name = 셀 · measured_utc = 리포트 생성일)이 가리키는 lite warm completed ≥ 1 | 리포트 생성일 |

어느 것도 없으면 **차단** `HINT_QUALIFICATION_UNOBSERVED`. 같은 셀 id 의 다른 회차 관측 · 선언 JSON 은 자격이 아니다.

## 3. 발행 가능 시점 (층별)

| 층 | 무엇 | 판정 | 집행 |
|---|---|---|---|
| **A** | **그 실행 평면의 재현 입력** — 트리플렛 3 · 쓰인 빌드 레시피 · 기동 방법 | **항상 필수**(선언으로 면제 불가) | `hintlib/artifacts.py` |
| **B** | 승격 tier — `full_benchmark` · `hint_map_only`(지도 발행 통로 · plan·devlog·testlog 필수) | 게이트 판정 | `completion_gate` `BASE_REQUIRED_EVIDENCE` |
| **C** | 승격 기록(work-manifest · promotion_target) | **도구가 쓴다 — 손 JSON 0**(identity · runtime · pii · promotion_target) | `evidence.drive_publisher` · `evidence_publisher set-promotion-target` |
| **측정** | 성능 수치 | **관측 게재만** — `baseline`·권고 승격 금지 · lite 는 결손 기재 · `verdict=FAIL` 은 §3.2 perf_waiver | `completion_gate` + 00 `OBSERVATION-ONLY`/`PERF-WARNING` 린트 |

**A 층 = 평면별 재현 입력 · "쓰인 것만"**(v6 · plan §4.5 · 원인 3 · F2·F3). 경로 규약상 거기 있던 파일이 아니라 **이 셀의 빌드·서빙이
실제로 쓴 것**만 싣는다. 평면(`identity.plane ∈ {docker, native}`)은 serve proof producer 가 명시한 신호로 정한다(신호 부재 = 추측 ✗ ·
`HINT_PLANE_UNDERIVABLE`). **v7(U1)부터 두 평면의 슬롯 집합은 같다** — native 는 자기 원천 이미지(serve proof
`wheelhouse.source_image_tag`)의 레시피 · 패치를 싣고, 실행되지 않은 자리는 생성 + 파일 단위 표시(§3.3)다.

| 평면 | `build_recipe` · `build_patch_pre/post` | `compose`(기동) | 고유분 |
|---|---|---|---|
| docker | **쓰인 Dockerfile 1종**(선택자 순위 = 빌드 원장 `dockerfile` → 셀 env `BUILD_DOCKERFILE` → compose 기본값 · 태그는 가변 포인터라 휴리스틱 ✗ · 2026-09-04) + **그 Dockerfile 이 COPY 하는 파일** · 적용된 빌드 패치 | compose · `serve_runner.sh`(asset 정본 · 마운트본과 바이트 동일 확인) · `arm_patch.sh` · env **형상** 템플릿(§7.2) · (multi) `sub_recipe.json` | — |
| native | **원천 이미지** 기준의 쓰인 Dockerfile + requirements · 원천 이미지 빌드 원장 기준 적용 패치(그 이미지가 실제로 빌드·관측됐으므로 `verified`) | Docker 셀과 **같은 렌더러**에 이 셀의 트리플렛을 넣어 생성한 compose · `serve_runner.sh` · env 형상 · `sub_recipe.json` = `generated-unverified`(renderer) | `native-install.sh` · `pip freeze` · native 기동 기록(`verified` · 기록이 없으면 `HINT_MISSING_NATIVE_LAUNCH`) |

- **D-a(2026-09-22 통합 결정)**: 규칙은 "wheel 트랙에서만 requirements" 가 아니라 "**쓰인 것만**" 이다. 소스빌드도 `requirements.txt` 를
  싣는다 — `Dockerfile.source-build` 가 그것을 `/etc/pip/constraint.txt` 로 COPY 한다(태그2 이미지 history 의 `COPY requirements.txt`
  층으로 확인). 빼면 수신자의 빌드가 COPY 에서 죽는다. 목록은 손으로 적지 않고 Dockerfile 의 COPY 줄에서 파생한다.
- **싣지 않는 것**: 다른 평면의 파일 · 쓰이지 않은 Dockerfile(예: 소스빌드 셀의 wheel 스켈레톤 — 46/46 옛 태그가 같은 blob 을 실었다) ·
  자기게이트로 **skip** 된 빌드 패치(F3 — 50·55 가 "적용 3-signal" 로 실렸다) · 쓰인 Dockerfile 이 COPY 하지 않는 패치 디렉터리
  (`excluded_by_recipe` 로 기재 · wheel 트랙은 build_patches* 를 한 번도 실행하지 않는다 · 2026-09-04).
- **적용 판정의 증거 순위**(X9·X12): ① serve 시점 캡처 원장(attestation v2) ② 메인 로컬 이미지 안 빌드 원장
  `/opt/easy-vllm/build_ledger.json`(측정 digest 로만 · 서브 스캔 ✗) ③ 원장 없는 옛 이미지 = `applied_set.status: unobservable` +
  라벨된 재구성(docker history build-arg × 패치 자기게이트) — 재구성이 skip 이라 말한 패치는 싣지 않는다 ④ 재구성도 못 하면 싣되
  01 표에 `적용 미관측`. 각 순위의 탐침 결과는 `applied_set.probes[]` 에 남는다(조용한 강등 ✗).
- **post 패치(공유 이미지 arch-enablement)**는 적용됐으면 싣고 `model-trigger` 헤더로 "이 모델의 요구가 아니라 공유 이미지의 일부" 라벨을
  단다(X10 · 이미지 네이밍 불변식 "이미지 하나가 모든 모델").
- **3신호 대사**(파일 × 적용 증거 × 선언)는 유지한다. 적용 증거 1순위가 빌드 원장이 됐고, 선언은 01 §1.2 의 슬롯별 적용 사유다
  (`PAYLOAD.slots[*].rationale · confidence`). 관측원이 없으면 `None`(모름)이지 `False`(없음)가 아니다(감사 ⑦ 거짓 음성 실증) —
  그 경우 confidence 가 내려가고 그 사실이 산출물에 표시된다. 옛 "이식 기록 `PROVENANCE.json` = 적용 증거" 규칙은 삭제했다(K3 — 이식
  기록은 '이식했다' 는 증거이지 '이 빌드가 실행했다' 는 증거가 아니다).

**A 를 왜 안 여는가**: 트리플렛·빌드 레시피·기동 방법이 없으면 재현이 **원리적으로** 불가능하다. 그건 "모르는 것"이 아니라 "지도가
아닌 것"이다. 결손 기재는 *모르는 것*에 쓰는 도구다.

**결손 가시성은 세 곳이다**(하나라도 빠지면 수신자가 비교할 수 없다): ① 00-hint 결손 블록(측정 결손은 03 §3.3 도) ②
`PAYLOAD.json.missing[]` ③ 카탈로그 `결손` **파생 컬럼**(`hints/index.json`·`HINTS.md`). 태그 **이름**에는 등급을 새기지 않는다 —
이름은 불변인데 결손은 재발행으로 바뀌고, 새기는 순간 이름이 거짓이 된다. 결손 코드는 `hintlib/evidence.py` `MISSING_CODES` 사전의
것만 쓴다(미등재 코드 = `HINT_MISSING_CODE_UNREGISTERED`).

### 3.0 벤치 절은 손저작하지 않는다

`03-benchmark.md` §3.2 부하 곡선은 `<!-- BENCH_SECTION -->` 다음 줄부터 다음 챕터 헤딩 직전까지가 **기계 렌더**다
(`scripts/render_bench_section.py` 가 바인딩된 리포트 또는 스윕 색인을 결정론 파싱). LLM 은 이 표의 숫자를 옮기지 않는다. 린터가
같은 원천으로 in-process 재렌더해 diff 0 을 요구한다(`HINT_BENCH_SECTION_DRIFT` · 원천 없이 곡선만 있으면
`HINT_BENCH_SECTION_UNVERIFIABLE`) — 옛 계약은 `--verify` 를 적어 두고 **부르는 게이트가 없었다**(K6). 닫힘 표지를 두지 않는 이유:
`render_bench_section.py --verify --section 03-benchmark.md --report <리포트>` 의 절 추출 계약(제목 ~ 다음 `## `)과 바이트가 맞아야
수신자도 같은 명령으로 확인할 수 있다(2026-09-22 리뷰: FACT 닫힘 표지가 절에 딸려 들어가 모든 신 형식 페이로드에서 `--verify` 가
실패했다).

### 3.0.1 lite 만 잰 셀의 통로 (2026-09-14 · plan_26091407 §4.5 · 사용자 결정 Q4·Q10)

v5 의 C행("B 가 열리므로 도달 가능하다")은 처음에 **승격 게이트까지만** 참이었다 — 봉인의 바인딩 판정자가 `hint_map_only` 를 몰라
죽었고(audit_26091323 §1), lite 는 바인딩할 문서를 내지 않았다. 2026-09-14 에 통로가 끝까지 섰고 v6 가 그것을 그대로 잇는다.

- **바인딩 대상**: 인증서는 explicit ∧ PASS 전용(벤치 스킬 발행 조건)이라 lite 셀에는 구조적으로 없다. `hint_map_only` 는 bench_report 를 묶는다 — 선언된
  lite-only 셀은 **경량 리포트**(`lite_bench.sh --publish-report` → 헤더 `mode: lite` · `evidence_publisher publish-lite-report`),
  반복 불성립으로 강등된 셀은 그 스윕의 리포트(`evidence_publisher init --downgrade-from full_benchmark --downgrade-reason
  <run_failed|blackbox_kill>`)다. 재분류는 **대조**다 — 바인딩된 리포트의 측정 구성 표가 같은 강등 사유를 말할 때만 열린다.
  캠페인 셀이면 이 구동은 `hint.py publish` 가 한다(§6).
- **footer 바인딩 판정자는 하나**(`evidence.binding_artifact_path` · 2026-08-01/08-24/09-14 · 단일 결정자): 인증서가 있으면 인증서가
  이긴다. 없으면 perf_waiver → hint_map_only → explore 순서로 bench_report 를 연다. 셋 다 아니면 묶을 것이 없어 **커밋 전에** 멈춘다
  (`HINT_CERTIFICATE_BINDING_ABSENT`). 묶은 것의 **종류**는 footer v2 `bench_kind`(certificate | bench_report · 파일 이름에서 파생 · §7.3)가
  말한다 — v1 의 `certificate_ref` 가 REFUTE · lite 셀의 리포트를 인증서라 부르던 거짓을 새 태그에 새기지 않는다.
- **인증서 요구 = 인증서가 발행되는 판정에서만**(2026-09-29 · plan_26092908 §4.8 V11④ · 소유 `completion_gate`): `full_benchmark` 의
  인증서 요구는 explicit ∧ PASS 에만 걸린다. weak · explore 의 PASS 와 REFUTE 는 벤치 스킬이 인증서를 내지 않으므로 bench_report 의
  판정 표(`verdict` · `루브릭 권한` 행)가 요구 증거이고, 게이트가 그 표를 manifest rubric carrier(발행기가 `verdict.json` 에서 옮긴 권한 ·
  floor · ratio · 출처 표시)와 교차검증한다. 권한의 출처 표시가 없으면 종전대로 인증서를 요구한다(fail-closed). 발행기는 full_benchmark
  발행에 `verdict.json` 을 넘길 수 없으면 첫 쓰기 전에 멈춘다(`HINT_VERDICT_JSON_ABSENT` — 권한이 게이트에 닿지 않으면 인증서 요구가
  되살아나 사람이 인증서를 손으로 만든다 · DS4F 2026-09-28 선례).
- **등급 기재**: 측정 구성 표의 `bench_mode` · 도구 · 반복 N · `downgrade_reason` 을 `PAYLOAD.measurement_config` 에 싣는다(D-f ·
  아래). 배포 평면에는 **열거·수치 칸과 출처 포인터**(`bench_report(<파일명>)`)만 싣는다 — 리포트의 자유 서술 `*_source`(블랙박스
  events 절대경로가 섞일 수 있다)는 바인딩된 리포트에 남는다. lite 로 **기재된** 측정만 `BENCH_MODE_LITE` 를 받는다 — 모름을 lite 로
  접으면 합성이다. 그 구분은 카탈로그 `bench_mode` 칸이 말한다(`full` · `lite(선언)` · `lite(강등·<사유>)` · `lite` · `미확정` ·
  `미기재` · `미수령`).
- **레시피 세그먼트**는 더 이상 `--lockset` 으로 대조하지 않는다 — 전 축을 셀 증거에서 도구가 파생한다(§4). lockset 출처 표시는
  01 §1.3 값의 지위 후보의 근거로 읽힌다.
- **OBSERVATION-ONLY**: `hint_map_only` 면 00-hint 본문(주석 밖)에 마커가 있어야 한다 — 기계가 00 사실 블록에 렌더하고 린터가
  배포되는 본문에서 확인한다(`HINT_MAP_ONLY_OBSERVATION_MARKER_MISSING`). 선언만 받고 본문을 보지 않으면 그 선언은 배포물에 도달하지
  않는다(2026-08-01 선례).
- **D-f(2026-09-22)**: `PAYLOAD.measurement` = **측정 수치**(인증서 성능 칸 + 스윕 레벨) · `PAYLOAD.measurement_config` =
  `evidence.DISTRIBUTED_MEASUREMENT_KEYS`(bench_mode · bench_mode_kind · downgrade_reason · bench_tool · bench_tool_version · repeats ·
  repeats_completed) + `source`. SPEC §2.1 이 measurement 에 두었던 분포 키는 measurement_config 로만 간다(evidence 의 분리 그대로).

### 3.0.2 판정의 기계 표면 (v7 · 2026-09-29 · plan_26092908 §4.4 · V1 · V2)

v6 는 측정 칸을 **인증서에서만** 옮겼다 — 인증서는 explicit PASS 에만 나오므로 REFUTE 셀(D1 · N1)의 `PAYLOAD.measurement` 에는 판정도
수치도 없었고, REFUTE 와 "인증서 없는 PASS" 가 같은 `HINT_MISSING_CERTIFICATE` 한 줄로 보였다.

- `PAYLOAD.measurement` 는 **인증서 유무와 무관하게** 판정 원천(스윕 `verdict.json` > bench_report `## 판정` 표 > 발행 기록 `benchmark`)에서
  채운다: `verdict ∈ {PASS, REFUTE, OBSERVATION-ONLY}` · `decode_tps_conc1` · `floor_tps` · `ratio_M_over_primary` · `rubric_authority` ·
  `benchmark_mode` · `accept_len` + 키별 출처 `sources{키: 출처}`. 판정이 한 번도 내려지지 않은 lite-only 셀 = `OBSERVATION-ONLY`.
- 인증서가 있으면 인증서 칸이 이기고, 같은 측정의 판정 원천이 다른 값을 말하면 **죽는다**(`HINT_MEASUREMENT_VERDICT_MISMATCH` — 두 기록이
  다른 측정을 말한다 · 고르지 않는다). 어휘 밖 판정 = `HINT_MEASUREMENT_VERDICT_UNKNOWN`.
- 표면 네 자리: ① `PAYLOAD.measurement.verdict` ② 00 `FACT:grade` 의 판정 행(필수 · 출처는 행 안의 출처 칸) ③ 00 머리 배너 — REFUTE 는
  `OBSERVATION-ONLY` 배너와 같은 자리(수치는 관측이지 baseline · 권고가 아니다) ④ 카탈로그 `판정` 열(§6.4). 무인증서 배너는 explicit PASS 의
  결손에만 붙고, 비발행 판정은 `measurement.verdict` · `sources.verdict` 로 설명한다.

### 3.1 필수는 여정 — 부재는 기재 (v4 · 2026-09-06 plan_26090616 Q7/Q8 · 사용자 결정)

> v2 는 서빙 증거와 lite 지표 **둘 다**를 필수로 걸었다. 그 결과 2026-09-05 캠페인에서 서브는 완주했는데(45.28 t/s PASS) 태그가 나가지
> 못했고 기대 3종 중 2종만 발행됐다. 수신자에게 "발행 안 됨" 은 **"시도된 적 없음"** 과 구분되지 않는다.

v4 는 "필수는 **여정 정보 하나**, 나머지 부재는 기재" 로 절단했다. v6 는 그 원칙을 잇되 부재로 발행을 막는 자리를 **닫힌 목록**으로
적는다 — 목록 밖의 부재는 기재다. 각 자리의 code 는 §5 표가 든다.

| 부재로 막는 자리 | 왜 기재로 대신할 수 없나 |
|---|---|
| §2 의 **관측**(serve_proof · post_health+bench · lite raw/warm) | 계약의 유일한 위협(서빙 실패를 성공으로 위장)이 바로 이 부재다 |
| **A 층**(트리플렛 · 쓰인 Dockerfile · compose) | 재현이 원리적으로 불가능하다 — "모르는 것" 이 아니라 "지도가 아닌 것" 이다 |
| **여정**(02 의 `kind: wall` ≥ 1 · 계보 문서 ≥ 1 · 필수 챕터·필드) | 없으면 태그가 아무 값도 나르지 않는다 |
| **서사 증거**(셀의 plan · devlog · testlog 포인터) | 발행기(`evidence_publisher`)와 게이트가 이 셋을 필수 증거로 요구한다 — "문서 완료 후" 의 기계 판정 |
| **footer 로 묶을 계측 산출물**(인증서, 또는 인증서가 구조적으로 없는 세 경로의 bench_report — §3.0.1) | footer v2 의 `bench_ref` 가 빈 주소가 된다 |
| **이름·노드 축의 증거**(§3 A · §4 — 이름 축의 원문 · 평면 신호 · 노드 축 원천(측정 노드 또는 배정)) | 이름은 불변이다 — 추측한 축은 영구히 거짓 이름이 된다 |
| **승인 기록**(§6.1) | 무인 자동 태깅 ✗ |
| **`pii_terms.txt`**(빈 파일은 "있음") | 리터럴 없이 PII-clean 을 인증하지 않는다(아래 ⚠) |

그 밖의 차단은 양성 검출이다(§5). 여정 = Agent 가 읽을 수 있는 형태로 *무엇을 시도했고 어디서 멈췄는지*. 없으면 그 태그는 아무 값도
나르지 않으므로 fail-closed 다. 판정은 템플릿 린터가 한다(§8): 02 에 `kind: wall` 블록 ≥ 1(`HINT_JOURNEY_WALL_ABSENT` — "필수는 여정
정보 하나") · 필수 챕터·필드 · 미저작 자리표시(`<<AGENT:`)·PROMPT 잔존 0 · 계보 문서 ≥ 1(`HINT_LINEAGE_EMPTY`). 옛 집행자로 적혀 있던 `selftest_hint_gate.py` 는 **실행자 0** 이었다(아무 러너에도 등록되지 않음) — 그 단언은
각 hintlib 모듈 selftest 로 옮겨 `hint.py --self-test` 가 전종 실행한다.

- **부재는 차단이 아니라 기재다**(인증서 · lite · 슬레이브 attestation · env 형상 · 빌드 원장 · 캠페인 인스턴스 purge 등 —
  `MISSING_CODES`). 적어야 수신자가 *무엇을 모른 채 소비하는지* 안다.
- **인증서 부재는 발행기의 책임이 아니다.** 인증서는 full · explicit ∧ PASS 일 때만 나오며 그 발행은 `adversarial-benchmark` 가 소유한다.
  "캠페인을 종료했다" 와 "hint 를 발행해야 한다" 는 독립 사건이다(사용자 결정). 인증서가 없는 셀은 perf_waiver · `hint_map_only` ·
  explore 경로로 bench_report 를 묶고 `HINT_MISSING_CERTIFICATE` 를 기재한다.
- **적히지 않은 부재는 차단이다** — 면제되는 것은 "없다" 가 아니라 "**없다고 적혀 있다**" 이다. 예: 서빙 노브 후보가 0개인데
  트리플렛 부재가 기재되지 않았으면 `HINT_VALUE_STATUS_CANDIDATES_ABSENT`.
- ⚠ **`pii_terms.txt` 부재는 더는 "선언된 결손" 이 아니다**(K11 · 2026-09-21) — 옛 계약은 기재 후 통과라 적었는데 트리 발행기는
  죽었다(의미가 도구마다 반대). 배포면은 리터럴 없이 PII-clean 을 인증하지 않는다 → 차단 `HINT_PII_TERMS_ABSENT`. 리터럴이 정말
  없으면 빈 파일로 그 사실을 선언한다.

### 3.1.1 full ⊇ lite 불변식

full 벤치는 lite 의 **상위집합**이어야 한다. 교집합이면 lite 로 잰 모델과 full 로 잰 모델의 지표 열 집합이 달라져 조건별 비교가
깨진다. 구조적 보장: `sweep_bench.sh` 가 `lite_bench.sh` 를 **실제로 실행해서 포함**한다(병렬 목록 유지 ✗). 집행:
`scripts/conformance_full_superset_lite.py`(adversarial-benchmark 소유 · 여기서는 인용만).

### 3.2 성능 REFUTE — loop-until-done 과 사람의 중단권 (v2.1)

통상 성능 REFUTE 는 여기서 끝이 아니다 — 서빙전략을 재수립하고 벤치마커의 측정 평면을 넓혀 가며(마지막 평면은 사람이 수동 수집한
정보까지) **loop-until-done** 으로 반복한다. 그 루프를 중간에 멈출 수 있는 것은 **사람의 지시뿐이다.** 사람이 그 지시를 내린 경우,
hint 는 발행하되 **경고 플래그를 배포물에 박는다**: *서빙 성공은 했으나 성능이 기대 이하다. 이 자료를 서빙전략 수립에 쓸 때는
적대적 검증이 필요하다.*

| 층 | 규칙 (fail-closed · 에이전트가 스스로 열 수 없다) |
|---|---|
| work-manifest | `benchmark.perf_waiver` = `authorized_by` · `authorized_at_utc` · `instruction` · `warning_flag` **4필드 전부** 비어 있지 않아야 유효 |
| `completion_gate.py` | waiver 유효 시에만 `verdict != PASS` 로도 승격 허용(`BENCHMARK_VERDICT_WAIVED`). 하나라도 비면 `BENCHMARK_PERF_WAIVER_MALFORMED` |
| 템플릿 린터 | waiver 가 있으면 **00-hint 본문**에 `PERF-WARNING` 마커와 `warning_flag` 원문이 둘 다 있어야 한다(`HINT_PERF_WARNING_MISSING`). 경고 없는 waiver 는 단순 게이트 우회다 |
| 인증서 | **여전히 explicit PASS 때만 발행**한다. waiver 는 승격만 열 뿐 *"성능이 검증됐다"* 는 주장을 만들지 못한다 — footer 는 bench_report 를 묶는다(`bench_kind: bench_report`) |

**설계 의도**: 이 예외는 에이전트가 추론으로 열 수 없는 **positive key** 다. 얻는 것(지도 배포)과 치르는 것(경고)이 같은 트랜잭션 안에 있다.

### 3.3 native 구조 통일 · 파일 단위 검증 표시 · 관련성 (v7 · 2026-09-29 · plan_26092908 §4.2 · §4.3 · U1 · U1+ · U2)

- **검증 표시는 파일 단위**다. 모든 슬롯 파일 레코드(`PAYLOAD.slots[*].file_records[]`)에 `verification ∈ {verified, generated-unverified}` ·
  `generated_by ∈ {cell-run, renderer, agent}` · `verification_basis` 를 둔다. 표시 자리는 **셋이고 일치해야 한다**: ① 기계 필드 ② 파일 첫 줄
  헤더 주석(파일 형식의 주석 문법 · shebang 이 있으면 둘째 줄 · 문구 `⚠ generated-unverified — <사유> · 생성: <generated_by>` · 주석이 없는
  JSON 은 최상위 키 `_verification`) ③ 01 적용 판정 표의 `검증` 열. 불일치 = 차단(`HINT_VERIFICATION_MARK_MISMATCH` · 어휘 밖 · 헤더 부재 =
  `HINT_ARTIFACT_MARK_INVALID` · 경고 키를 넣을 수 없는 JSON = `HINT_ARTIFACT_HEADER_UNSUPPORTED`).
- **생성 순서 = 렌더러 우선, Agent 는 폴백**. native 의 compose · `serve_runner.sh` 는 Docker 셀과 같은 렌더러에 이 셀의 트리플렛을 넣어
  만든다(`generated-unverified/renderer`). 렌더러로 만들 수 없는 자리는 생성하지 않고 **Agent 저작 요청**(`agent_requests[]`)으로 남긴다 —
  publish 가 경고 헤더만 든 빈 파일 + PROMPT 를 스캐폴드하고, 저작 Agent 가 본문을 채운다(부재 `HINT_AGENT_ARTIFACT_ABSENT` · 경고 밖 본문 없음
  `HINT_AGENT_ARTIFACT_UNAUTHORED` · 자리표시 잔존 `HINT_AGENT_PLACEHOLDER_RESIDUE` · artifacts 밖 경로 · JSON = `HINT_AGENT_REQUEST_PATH`).
- **관련성**(U2 · V8): 적용된 것은 다 싣고(이미지 바이트 재현) 파일마다 `relevance ∈ {required, inactive-inferred, unknown}` + `relevance_basis` 를
  둔다. 근거는 ① 패치 헤더가 선언한 대상(모델 아키텍처 · 기능 토큰) 대 셀 모델 config `architectures` ② 엔진 로그 발화 서명 — 둘 다 관측이면
  `required` / `inactive-inferred`, 하나라도 없으면 `unknown`(추론을 확정처럼 쓰지 않는다). 01 표에 `관련성` 열 · 00 §0.4 에 "이 모델에 필요한
  패치" 요약(required 목록 + unknown 수).
- **재현 보조 데이터**(V14): `build_context_map`(zip 슬롯 경로 → Dockerfile COPY · compose `env_file`/`volumes` 가 기대하는 경로)을 01 재현 절차의
  "슬롯 → 빌드 컨텍스트 매핑" 표와 README 요약으로 · env 형상 템플릿은 **키마다 자기 자리표시**(한 manifest 필드에서 여러 키가 파생되면
  `<derived:<field>→<KEY>>` + 실효값 표).
- **결손 코드**(V7 · 침묵 ≠ 없음): 서브 레시피 미관측 `HINT_MISSING_SUB_RECIPE` · 빌드 패치 적용 집합 미관측 `HINT_MISSING_APPLIED_SET` · native
  기동 기록 부재 `HINT_MISSING_NATIVE_LAUNCH`. 01 의 결손 문구는 `PAYLOAD.missing` 에서 **파생**한다(산문이 결손이라 적는데 결손 표에 없는 자리 0).
- 정문 도구(스모크 · 렌더러 · 워치독 · native 정문 · wheelhouse 도구)는 싣지 않는다(O2 = a) — `<저장소>@<커밋 SHA>:<경로>` 핀 포인터와 도구 없이
  재현하는 최소 절차를 01 에 기계 생성한다.

## 4. 이름 — 결정론부는 도구가, 꼬리는 Agent 가 근거와 함께 (v7 · U4~U7 · 옛 v6 D5~D8 "전량 파생" 의 개정)

```
hint/<vllm>/<model>/<arch>/q<quant>-len<n>-kv<dtype>[-<tail>][-t<YYMMDDHHMM>]
     └──────── 결정론(v6 그대로) ─────┘ └─ 결정론 3축 ─┘ └ Agent ┘ └ 원격·로컬 중복 시만 ┘
예: hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvauto-plemmap-eager
```

**왜 바꿨나**(2026-09-29 사용자 결정 U5): v6 는 레시피 6축을 모두 결정론으로 파생했다 — 셀을 가르는 노브가 늘 때마다 파생 규칙을 더해야 했고
("결정론 규칙이 한도 없이 늘어난다"), 같은 3축의 셀이 더 필요한 노브로 갈릴 때 이름이 모자랐다. v7 은 **q · len · kv 만** 결정론으로 두고,
나머지는 발행 Agent 가 근거와 함께 고르는 꼬리로 넘긴다. 규칙 목록을 늘리지 않고 **거짓 꼬리만** 막는다.

### 4.1 결정론부

| 세그먼트 | 문법 | 결정론 출처 |
|---|---|---|
| `<vllm>` | **빌드 입력**(D5). 업스트림 릴리스 태그로 빌드 → 그 태그에서 `v` 를 뗀 값: `M.m.p` + 선택 4번째 마디(`M.m.p.q`) + 선택 단계(`rc6`·`a1` …) + 선택 `.postN`(D-g). 커밋·nightly 핀 → `<직전 릴리스>-g<sha12>` | 소스빌드 = 셀 env `VLLM_REF`(+ `VLLM_REPO`) · wheel = `VLLM_VERSION` · native = wheel URL 의 SHA · 직전 릴리스 = 빌드 원장 `vllm.describe` 또는 이미지 안 `git describe --tags --match 'v*' --abbrev=0` |
| `<model>` | 체크포인트 슬러그 = HF repo 이름 또는 체크포인트 basename 소문자(`[a-z0-9][a-z0-9._-]*`) | 서빙 yaml `model:` · HF 카드 repo id(= 인증서 `model` 키 · 2026-08-15 IDENTITY_MISMATCH 선례) |
| `<arch>` | `<hw>-<G>g<N>n-<main\|sub\|cluster>-<target>[-<plane>]` — G = 노드당 GPU 수 · N = 노드 수 · target = `native` \| `sim-<hw>` · plane = 실행 평면 토큰(Docker = 없음 · native(비-Docker) = `-bare`) | `output/<t>/manifest.yaml`(gpu_model · gpus_per_node · nodes) · 측정 노드(인증서·스윕 `measured_node`) · 셀 config `target_gpu`(시뮬레이션 타겟 선언 여부) · 평면 = `artifacts.plane_of`(셀 env `IMAGE_TAG`·`BUILD_DOCKERFILE` ↔ native serve-proof `plane`) |
| `q·len·kv` | `q<quant>-len<n>-kv<dtype>` — **순서 고정 · 세 축 필수**(결정론). q = 체크포인트가 **선언한** 주 방식(`quant_method` · ModelOpt `MIXED_PRECISION` 은 층 우세를 보조 규칙으로 · U9) — 혼합 구성은 이름이 아니라 `PAYLOAD.identity.quant_composition[{scope, dtype, source}]` 와 00 §0.4 사실 블록에 싣는다 | 체크포인트 `quantization_config` · 서빙 yaml(`max-model-len` · `kv-cache-dtype`) |

**정규 어휘표 = `hints/vocab.json`**(추적 · tripwire 닫힌 목록 · 사람 편집). 결정론 축 `hw`·`quant`·`kv` 의 원문을 대소문자·공백만
정규화해 **정확 일치**로 토큰에 대응한다(부분 일치 ✗ — hw 는 에디션 구분이 목적이다). 에디션이 드러나지 않는 원문은 `hw_ambiguous`
에 두고 토큰에 합치지 않는다. 등재 근거는 같은 파일의 `_observed` 에 한 줄씩 남긴다. `quant_suffixes` 는 계보 필터·match 의 기반
슬러그(`base_slug`)를 만든다. 추가는 **이 파일 한 곳**에서만 한다(코드에 사본 ✗). `ple` · `graph` 키는 v7 에서 결정론 파생에서 빠졌고
지우지 않는다 — **꼬리 권장 토큰**(강제 ✗) 참고 목록이다(`naming.tail_candidates` 가 후보를 낼 때 쓴다).

- **어휘 밖 = 차단** `HINT_VOCAB_UNKNOWN`(remedy 가 어느 키에 추가할지 지목) · **파생 불가 = 차단** `HINT_AXIS_UNDERIVABLE`(remedy 가
  없는 증거를 지목). "읽지 못함"(`N/A` · 빈 값 · `<<FILL>>` 같은 템플릿 빈칸)은 값이 아니다.
- **D-h(포크)**: 업스트림이 아닌 `VLLM_REPO` 의 릴리스 모양 `VLLM_REF` 는 릴리스 이름을 쓰지 않는다 — 같은 이름을 쓰면 수신자는
  업스트림 릴리스로 읽는다. SHA 경로(`<직전 릴리스>-g<sha12>`)만 쓰고, SHA 가 없으면 차단한다. 업스트림 목록 =
  `naming.UPSTREAM_VLLM_REPOS`(tripwire · 사람 편집). naming facts 는 `vllm_repo` 를 싣는다.
- **평면 토큰**(2026-09-23 `plan_26092311` O-N1 = A): 이름 문법에 실행 평면 축이 없어 native 셀이 축이 같은 Docker 셀과 한 이름을
  원했다(N1 파생 이름 = D1 발행 태그 → `HINT_NAME_COLLISION`). 그래서 arch 끝에 선택 토큰을 둔다 — 토큰은 `hints/vocab.json` `plane`
  (docker = `""` 고정 → **Docker 이름은 옛·신 모두 바이트 불변** · native = `bare`)에서만 온다. 평면 판정은 `artifacts.plane_of` 한 벌이고
  출처는 `PAYLOAD.naming.axes.plane` 에 남는다. 평면 사실 부재 = `HINT_PLANE_UNDERIVABLE`(docker 로 추측 ✗ · `artifacts.plane_of` 가 낸다) ·
  어휘 밖 = `HINT_VOCAB_UNKNOWN`. **`native` 토큰(target)은 실 하드웨어라는 뜻이고 Docker 여부와 무관하다** — Docker 없이 돈 셀은 `-bare` 다
  (V9: 타 PC 옛 `…-main-native` 14건이 우연히 전부 비-Docker 라 오독 소지가 있었다 · 페이로드 README 가 읽는 법을 싣는다 §7.1).
- 엔진 자기보고(예 `0.29.0`) · wheel 메타 원문은 이름이 아니라 `PAYLOAD.naming.vllm_observed` 와 00 사실 블록에만 산다(F13: 출처가 셋으로
  갈려 rc6 소스빌드와 릴리스가 한 이름을 썼다).
- 축별 `{값, 출처}` 는 `PAYLOAD.naming`(`grammar: "v7"`)에 기록한다. 미리보기 = `hint.py name`(읽기 전용 · 기본 이름 + 꼬리 후보).
- **노드 축 자동 파생**(v7 · plan §4.8 V11③): `--node` 를 주지 않으면 셀 측정 TP > 노드당 GPU 이거나 멀티 serve proof · attestation 이 있을 때
  `cluster` 로 파생한다(멀티 셀 3/3 에서 사람이 `--node cluster` 를 명시하던 손작업의 제거). 모호하면 차단이다(추측 ✗).
- **노드 축 ↔ 측정 대조**(v5 §3.-2 · 2026-09-07 `plan_26090715` R4 — 그때까지 `measured_node` 와 arch 노드 축의 대조가 어디에도 없어
  서브가 잰 것을 main 태그로 봉인해도 게이트가 울리지 않았다): 발행 노드 축(`--node`, 없으면 위 자동 파생 → 인증서 `measured_node` → 캠페인 배정 순)이
  인증서 `measured_node` 와 다르면, `cluster` 인데 토폴로지가 multi 가 아니면, (캠페인 셀) 인증서 포인터의 `node_id` 가 그 축에 속하지 않으면 차단
  `HINT_ARCH_NODE_AXIS_CERT_MISMATCH`. 축을 파생할 원천이 없으면 `HINT_NODE_AXIS_UNDERIVABLE`(`--node` 로 준다) · main/sub 축이 셀의 배정 노드와
  다르면 `HINT_NODE_ASSIGNMENT_MISMATCH`. **부재는 막지 않는다**(부재 ≠ 불일치 — measured_node 가 없으면 배정으로 해소한다). 측정 노드
  어휘(`cluster` → 선언 노드)의 정본은 `campaign_template_validator.resolve_measurement_node` 한 벌이다. 입력은 명시한 캠페인 id 뿐이다 —
  `campaigns/ACTIVE` 를 읽으면 발행 시점마다 다른 것을 읽고(태그는 불변인데 입력이 흐른다) 캠페인 밖 발행이 활성 캠페인을 오독한다.
- 발행자는 **결정론부**를 입력하지 않는다(D8 · 2026-08-20: 발행된 32 슬러그 중 27종이 정본표 밖이었고 같은 모델이 철자로 2건 갈렸다).
- 왜 이 모양인가(날짜 박힌 불변식 · 정본 `hintlib/naming.py` docstring): 5세그먼트 레시피를 **맨 뒤**에 붙였다(2026-09-04 ·
  모델 슬러그 인덱스 `split("/")[2]` 가 그대로 산다) · arch 에 노드 축(2026-09-06) · GPU 수·노드 수 분리(D7 — 옛 `x2` 가 2노드와 2 GPU 두
  뜻이었다) · 결측 생략 금지와 어휘 정규화(D6 — `kvfp8`/`kvfp8e4m3` 분열) · 레시피 결정론 = q · len · kv 로 축소(2026-09-29 U5).

### 4.2 꼬리 — Agent 가 고르고, 도구는 거짓만 막는다 (U5 · U6)

- **무엇**: 이 셀을 **같은 q · len · kv 의 다른 셀과 가르는 노브**(PLE 모드 · spec · graph/eager 등). 빈 꼬리 허용 — 가르는 노브가 없으면 `[]`.
- **저작 자리**: 00 템플릿 `## 0.9 이름 꼬리` PROMPT + 기계 입력 draft `inputs/tail.json` = `[{token, meaning, evidence: {file, key, value}}]`(토큰 하나당
  한 행 · 빈 꼬리도 `[]` 로 **명시** — 파일이 없으면 `HINT_TAIL_ABSENT` · JSON 목록이 아니면 `HINT_TAIL_FORMAT`). publish 는 **기본 이름**(꼬리 없음)으로
  스캐폴드하고 `naming.tail_candidates` 의 후보를 사실 블록 표로 보인다(강제 ✗).
- **형식**(`HINT_TAIL_FORMAT` · fail-closed): 토큰 `[a-z0-9]+` · 토큰 ≤ 24자 · 꼬리 전체(`-` 연결) ≤ 64자 · 중복 토큰 ✗ · `t\d{10}` 모양 ✗(timestamp
  예약) · git ref 규칙 · **v6 뒤 3축 모양 그대로(`ple…-spec…-(graph|eager)`)면 거부**(그 모양은 기발행 v6 이름으로 읽힌다).
- **뜻**: 토큰마다 `meaning` 필수(`HINT_TAIL_MEANING_ABSENT`) → `PAYLOAD.naming.tail[]` 에 실린다.
- **근거 대조**(`HINT_TAIL_UNGROUNDED`): `evidence.file` 이 이 셀의 서빙 설정(서빙 yaml · 셀 env · 셀 config · native 트리플렛 — `evidence.tail_sources`
  가 publish 때 draft `inputs/tail_sources.json` 에 평탄 key→value 로 스냅샷) 중 하나이고, `key` 가 실재하며(yaml 중첩은 `a.b`), 값이 `value` 와
  일치(공백 · 따옴표 · 대소문자 정규화)해야 한다. 규칙 목록을 늘리지 않고 **거짓 꼬리만** 막는다.
- `hint.py lint` 가 같은 대조를 저작 중에 미리 보인다. 산문 · hint-event 에 태그 이름을 글자로 적으면 막는다(`HINT_TAG_LITERAL_IN_PROSE` — 이름은
  사실 블록이 소유한다 · 꼬리 확정 전 이름이 남는다).

### 4.3 이름 확정 · timestamp (U7)

- `continue` 가 커밋 **전**(부수효과 = draft 안뿐)에 확정한다: `tail.json` 대조 → `naming.compose_name(기본 이름, 꼬리)` → 로컬 · 원격 충돌 조회 →
  **이미 있으면** `-t<YYMMDDHHMM>`(이 발행의 publish `generated_utc` 를 KST 로 환산 · 문서 명명 SSOT 와 같은 시계 · 결정론)을 붙인다 → 그래도 있으면
  차단(`HINT_NAME_COLLISION` — 같은 분에 같은 셀 2건 · publish 를 새 시각으로 다시). 확정 이름은 모든 페이로드 파일의 태그 문자열(README ·
  PAYLOAD · PROVENANCE · 사실 블록)에 반영된다. 재실행 멱등(같은 입력 → 같은 이름).
- 봉인은 개명하지 않는다 — 이름이 페이로드 파일 안에 있으므로 봉인 직전 원격 재조회에서 그 사이 생긴 충돌은 차단만 한다(`tag.seal(remote=)`).
- **이름 충돌** 예외는 하나: 로컬 태그가 **이 draft 가 봉인한 것**이면 재개(멱등)다. 태그는 불변이고 같은 셀의 개정판은 timestamp 가 붙은 새
  이름이다(v6 의 "개정이 필요하면 축이 모자란 것" 은 timestamp 로 대체됐다 · 2026-09-29).
- **v6 호환**: v6 이름은 v7 파서에서 "결정론 3축 + 꼬리 `ple…-spec…-(graph|eager)`" 로 그대로 해석된다(`grammar: "v6"` 우선 인식). 카탈로그
  `문법` 열은 발행 형식 버전을 유지 표기한다(v6 태그 재판정 ✗ · P1).
- **match** 는 결정론부로 1차 매칭하고 꼬리 뜻(`PAYLOAD.naming.tail[].meaning`)은 보조 표시다(순위 판정 ✗).

## 5. 차단 / 기재 분류

**절단선(v4 · 2026-09-06)**: 차단은 **양성 검출**일 때만이다. 부재는 — 기재돼 있다면 — 통과한다. **등급은 메시지 문자열이 아니라
code 로 정한다**(감사 ② 2026-09-01: 태그 저자가 통제하는 footer 값에 특정 문자열을 넣으면 차단이 경고로 강등됐다). 판정은 `HINT_*`
code 로만 한다(봉인·push 거부는 `tag.problem_codes`).

| 검사 | 판정 | 대표 code |
|---|---|---|
| 발행 자격 관측 부재(§2) | **차단** | `HINT_QUALIFICATION_UNOBSERVED` |
| 사람 승인 부재(§6.1) | **차단**(부수효과 0) | `HINT_APPROVAL_ABSENT` |
| push 까지 가는 실행인데 중앙 권위 선언 부재(§6 · D8) | **차단**(커밋·봉인 전 · 부수효과 0) | `HINT_PUSH_CENTRAL_AUTHORITY_ABSENT` |
| 이름(결정론부): 어휘 밖 · 파생 불가 · 충돌 · 이름이 PII 스캔에 걸림(§4.1) | **차단**(publish 가 부수효과 전에) | `HINT_VOCAB_UNKNOWN` · `HINT_AXIS_UNDERIVABLE` · `HINT_NAME_COLLISION` · `HINT_TAG_NAME_PII` |
| 이름 꼬리: 파일 부재 · 형식(v6 뒤 3축 모양 · timestamp 모양 토큰 포함) · 뜻 없음 · 근거 불일치(§4.2) · timestamp 뒤에도 충돌(§4.3) | **차단**(continue 이름 확정 · 커밋 전 · 쓰기는 draft 안뿐) | `HINT_TAIL_ABSENT` · `HINT_TAIL_FORMAT` · `HINT_TAIL_MEANING_ABSENT` · `HINT_TAIL_UNGROUNDED` · `HINT_NAME_COLLISION` |
| 산문 · hint-event 에 태그 이름 리터럴(§4.2) | **차단**(린트) | `HINT_TAG_LITERAL_IN_PROSE` |
| 노드 축 ↔ 측정 불일치 · 노드 축 파생 불가 · 배정과 다름(§4) | **차단**(publish 가 쓰기 전에) | `HINT_ARCH_NODE_AXIS_CERT_MISMATCH` · `HINT_NODE_AXIS_UNDERIVABLE` · `HINT_NODE_ASSIGNMENT_MISMATCH` |
| 평면 파생 불가(§3 A · §4.1) | **차단** | `HINT_PLANE_UNDERIVABLE` |
| 파일 단위 검증 표시 3자리 불일치 · 어휘 밖 · 헤더 부재 · Agent 저작 파일 부재/미저작/경로(§3.3) | **차단** | `HINT_VERIFICATION_MARK_MISMATCH` · `HINT_ARTIFACT_MARK_INVALID` · `HINT_ARTIFACT_HEADER_UNSUPPORTED` · `HINT_AGENT_ARTIFACT_ABSENT` · `HINT_AGENT_ARTIFACT_UNAUTHORED` · `HINT_AGENT_REQUEST_PATH` |
| 판정 원천끼리 다른 측정을 말함 · 판정 어휘 밖 · full_benchmark 인데 `verdict.json` 을 넘길 수 없음(§3.0.1 · §3.0.2) | **차단**(publish · 첫 쓰기 전) | `HINT_MEASUREMENT_VERDICT_MISMATCH` · `HINT_MEASUREMENT_VERDICT_UNKNOWN` · `HINT_VERDICT_JSON_ABSENT` |
| A 층 부재 — 트리플렛 · 쓰인 Dockerfile · compose(§3 A) | **차단**(선언으로 면제 불가) | `HINT_TRIPLET_ABSENT` · `HINT_DOCKERFILE_ABSENT` · `HINT_COMPOSE_ABSENT` |
| 여정 부재 · 미저작 · PROMPT 잔존 · 필수 챕터·필드·발췌 부족(§3.1 · §8) | **차단** | `HINT_JOURNEY_WALL_ABSENT` · `HINT_AGENT_PLACEHOLDER_RESIDUE` · `HINT_PROMPT_RESIDUE` · `HINT_SECTION_UNAUTHORED` · `HINT_EVENT_FIELD_MISSING` · `HINT_EXCERPT_REQUIRED` |
| 계보 문서 0 | **차단** | `HINT_LINEAGE_EMPTY` |
| 셀 서사 증거(plan · devlog · testlog) 부재 · 모호(§6) | **차단**(발행기 구동 전 · 쓰기 0) | `HINT_NARRATIVE_EVIDENCE_ABSENT` · `HINT_NARRATIVE_EVIDENCE_AMBIGUOUS` |
| 발췌 무결성 · 출처 · 절 표지 · 줄 범위 · 상한 · 서명 대조(§8.3) | **차단** | `HINT_EXCERPT_MISMATCH` · `HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE` · `HINT_EXCERPT_SECTION_UNKNOWN` · `HINT_EXCERPT_SECTION_SHAPE` · `HINT_EXCERPT_LINES_OUT_OF_RANGE` · `HINT_EXCERPT_LINES_TOO_WIDE` · `HINT_EXCERPT_OUTSIDE_LINES` · `HINT_EXCERPT_SOURCE_LIMIT` · `HINT_EXCERPT_TOTAL_LIMIT` · `HINT_EXCERPT_SNAPSHOT_DRIFT` · `HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE` · `HINT_SIGNATURE_MISMATCH` · `HINT_SIGNATURE_TOO_SHORT` |
| 발행 전 독립 사실 검증 보고 부재 · 모양 결함(검증자 = 저작자 · 7부류 누락 · 다른 태그) · 열린 지적(§6.7) | **차단**(린트 0 뒤 · 봉인·커밋 전 · ref 쓰기 0) · 사람 면제만 탈출 | `HINT_FACTCHECK_ABSENT` · `HINT_FACTCHECK_INVALID` · `HINT_FACTCHECK_OPEN` · `HINT_FACTCHECK_WAIVER_INVALID` |
| brief 수치가 사실 블록에 없음 · 사실 블록에 절 자리표시(`§3.x`) · 결과 출처 어휘 밖 · 봉인 뒤 저작 자기점검 잔존(§8.6) | **차단** | `HINT_BRIEF_NUMBER_UNGROUNDED` · `HINT_FACT_PLACEHOLDER` · `HINT_RESULT_SOURCE_UNKNOWN` · `HINT_SELFCHECK_RESIDUE` |
| 사실 블록을 손댐 · 벤치 절 숫자를 손댐 · 저작 중 템플릿 지시를 고침 | **차단** | `HINT_FACT_DRIFT` · `HINT_BENCH_SECTION_DRIFT` · `HINT_PROMPT_TAMPERED` |
| 값의 지위 커버리지 · 필요조건의 근거(§8.4) | **차단** | `HINT_VALUE_STATUS_GAP` · `HINT_VALUE_STATUS_VALUE_MISMATCH` · `HINT_VALUE_STATUS_CANDIDATES_ABSENT` · `HINT_VALUE_STATUS_REQUIREMENT_UNGROUNDED` |
| 00 배너 3줄 부재 · `OBSERVATION-ONLY` · `PERF-WARNING` 부재 | **차단** | `HINT_BANNER_ABSENT` · `HINT_MAP_ONLY_OBSERVATION_MARKER_MISSING` · `HINT_PERF_WARNING_MISSING` |
| **PII** — 페이로드 트리 · 파일 이름 · 커밋 메시지 · annotation(배포 4종 + 리터럴) · `pii_terms.txt` 부재 | **차단** | `HINT_PAYLOAD_PII` · `HINT_PAYLOAD_PATH_PII` · `HINT_COMMIT_MESSAGE_PII` · `HINT_TAG_PII` · `HINT_PII_TERMS_ABSENT` |
| 커밋 author/committer · tagger 가 **합성 신원이 아님**(동일성 판정 · §7.4) | **차단** | `HINT_COMMIT_IDENTITY_NOT_SYNTHETIC` · `HINT_TAGGER_NOT_SYNTHETIC` |
| env 형상화 뒤 잔존한 4종 패턴(2차 백스톱 · 덧칠 ✗) | **차단** | `HINT_ENV_SHAPE_PII` |
| 페이로드 트리가 allowlist 밖(§7.1) | **차단** | `HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST` |
| 앵커의 부모가 형식 마커를 가진 안내 커밋이 아님(소스 트리 커밋 포함) · 앵커 트리에 PAYLOAD 없음 · PROVENANCE.tag 불일치(§6.5) | **차단** | `HINT_ANCHOR_PARENT_NOT_GUIDE` · `HINT_ANCHOR_PAYLOAD_ABSENT` · `HINT_ANCHOR_PROVENANCE_TAG_MISMATCH` |
| 원격 안내 커밋의 형식 ≠ 발행기 형식(마커 없는 옛 안내 포함) · 안내 커밋 미지정 · 그 사이 안내가 바뀜 · 원격 조회 실패 · push 까지 가는 발행에 안내 지정(§6.5) | **차단**(커밋 전) | `HINT_BRANCH_FORMAT_MISMATCH` · `HINT_BRANCH_GUIDE_REQUIRED` · `HINT_BRANCH_GUIDE_MOVED` · `HINT_REMOTE_QUERY_FAILED` · `HINT_GUIDE_PIN_REQUIRES_NO_PUSH` |
| footer 가 v1 인 신규 봉인 · `bench_kind` 가 `bench_ref` 이름 파생과 다름(§7.3) | **차단** | `HINT_EVIDENCE_BINDING_VERSION` · `HINT_EVIDENCE_BINDING_MALFORMED` |
| LINEAGE 수신자 요약 > 20KB · 봉인 출처 스냅샷 모양 결함(§8.1 · §6.3) | **차단** | `HINT_LINEAGE_SUMMARY_TOO_LARGE` · `HINT_SEALED_SOURCES_SHAPE` |
| footer 로 묶을 계측 산출물 없음 · promotion_target 불일치 | **차단**(커밋 전 판정) | `HINT_CERTIFICATE_BINDING_ABSENT` · `HINT_PROMOTION_BINDING_MISMATCH` |
| 커밋 뒤 draft 가 바뀜(재실행) | **차단** — 커밋된 페이로드는 이름과 한 몸이다(같은 이름으로 다른 내용 ✗) | `HINT_DRAFT_CHANGED_AFTER_COMMIT` |
| push 뒤 원격 태그 오브젝트 ≠ 로컬 · 원격에 같은 이름의 다른 오브젝트 | **차단**(강제 ✗) | `HINT_REMOTE_SHA_MISMATCH` · `HINT_REMOTE_TAG_CONFLICT` |
| 카탈로그 자동 커밋 실패(훅 · 신원) | **차단**(push 뒤 · 재실행이 잇는다 · 우회 ✗) | `HINT_CATALOG_COMMIT_FAILED` |
| 봉인 **뒤** 출처 문서가 바뀌어 생긴 출처 의존 린트(발췌 · 서명 불일치 등 · §6.3) | **INFO**(FAIL ✗ · 구조 검사는 그대로 FAIL) | `INFO: 출처 변경됨(봉인 뒤)` |
| skip 된 빌드 패치 · 쓰이지 않은 Dockerfile · 다른 평면 파일(§3 A) | **싣지 않는다**(차단이 아니라 제외 · 사유 기재) | `excluded_by_recipe` 등 |
| 원장 없는 옛 이미지 · 재구성 불가 패치 | 통과 + **기재**(`unobservable` · `적용 미관측`) | `HINT_MISSING_BUILD_LEDGER` |
| **기재된** 결손(`MISSING_CODES` — 인증서 · 벤치 리포트 · 스윕 · lite · 슬레이브 attestation · env 형상 · 캠페인 인스턴스 · (v7) 서브 레시피 · 적용 집합 · native 기동 기록 …) | 통과 + **기재** | `HINT_MISSING_*`(v7 신설 `HINT_MISSING_SUB_RECIPE` · `HINT_MISSING_APPLIED_SET` · `HINT_MISSING_NATIVE_LAUNCH`) · `BENCH_MODE_LITE` |
| 과거 · 타 PC 태그의 결함(옛 문법 · 로컬 오브젝트 부재 · lightweight) | **이번 발행을 막지 않는다**(D10 · §6.3) · 카탈로그에 정직한 행으로만 | `absent-local` · `not-annotated` |

PII 는 배포 산출물이므로 `hintlib/pii.py` 의 `GENERIC_PII` 4종 전부와 `.claude/pii_terms.txt` 리터럴이 강제된다(`.claude/rules/docs.md`
§PII 스캔 적용 범위). v3 부터 태그 본문뿐 아니라 **페이로드 트리 전량**이 대상이고(감사 N-3), v6 부터는 **커밋 메타데이터**도 대상이다
(K1: 옛 페이로드 커밋 57/57 이 ambient `git config` 신원으로 찍혀 있었다 — 4종 스캔은 커밋 메타를 보지 않았다).

## 6. 발행 절차 — 셀 1개 = 태그 1개 · 단일 진입 (D9)

```
hint.py publish  --campaign <id> --cell <cell> [--node main|sub|cluster] --generated-utc <UTC>   ← --node 는 보통 생략(§4.1 자동 파생)
  ① 셀 증거 수집(명시 id 만 · campaigns/ACTIVE 를 읽지 않는다)   ② 발행 자격 관측(§2)
  ③ 기본 이름(결정론부 · §4.1) 파생 · ref 형식 · 이름 PII · 충돌 · 발행 기록 시각 판정(채택 · 아래)   ← 여기까지 부수효과 0
  ④ evidence_publisher 구동(init → set-narrative → append-raw(simlog) → publish-benchmark | publish-lite-report →
     finalize · identity·runtime·pii 입력은 도구가 관측해 draft inputs/ 에 쓴다) → 승격 게이트 사전 확인(hint_finalize)
  ⑤ 측정 도구 스냅샷(draft `inputs/sources/<도구>@<rev12>`) · 측정 env 관측 → 계보(전체 `inputs/LINEAGE.full.json` · §8.1)
  ⑥ 산출물 적용 판정(§3 A · §3.3 · 이미지 탐침 기본 · native 도 같은 슬롯)  ⑦ 사실 블록을 채운 스캐폴드 · PAYLOAD · PROVENANCE(앵커 비움) ·
     README · 꼬리 후보 · `inputs/naming_base.json` · `inputs/tail_sources.json` · state.json → **정지**.
     출력 = 채울 PROMPT 목록 · LINEAGE 읽기 목록 · 다음 명령
(저작)  Agent 가 PROMPT 절을 산문 · hint-event · 원문 발췌로 채운다(§8) · 00 §0.9 이름 꼬리 → `inputs/tail.json`(§4.2) · Agent 저작 요청 파일(§3.3) —
        hint.py excerpt · hint.py refresh(파생 요약 미리 보기 · draft 안 쓰기만 · 커밋 뒤 `HINT_DRAFT_ALREADY_COMMITTED`) · hint.py lint(부수효과 0 ·
        꼬리 대조 포함) 가 0 이 될 때까지
(검증)  저작자가 아닌 Agent 가 산문과 FACT 블록의 사실 주장 전부를 출처와 대조해 <draft>/inputs/factcheck.json 을 쓴다(§6.7 · 항목마다
        `target: prose|fact`) → 산문은 저작자가 고치고 · FACT 는 생산자를 고친 뒤 `hint.py refacts` → lint 0
hint.py continue (--campaign <id> --cell <cell> | --draft <draft>) --generated-utc <UTC> [--remote origin] [--no-push]
                 [--guide-commit <SHA>]                                        ← --no-push(오프라인 재생) 전용
                 [--approved-by "<사람 발화 전사>" --approved-utc <UTC>]       ← 캠페인 밖 발행만
                 [--factcheck-waiver "<사람 발화 전사>"]                        ← 사람이 사실 검증을 면제했을 때만
  ① 승인(§6.1 · 부수효과 0 · 면제 전사 모양도 여기서)
  ② 이름 확정(§4.3 · 꼬리 대조 → 기본 이름 + 꼬리 → 로컬·원격 중복이면 `-t<YYMMDDHHMM>` → 페이로드 파일의 태그 문자열 갱신 · draft 안만)
  ③ 안내 커밋 조회(§6.5 · 원격 refs/heads/hint 를 ls-remote 로 읽기만 · 형식 마커 대조 · state 에 기록)
  ④ 기계 사실 갱신(승인 · 01 §1.2 적용 사유 → slots.rationale·confidence · 사실 검증 요약 → 00 메타) → 파생 블록 refresh → 린트 →
     **사실 검증 게이트(§6.7)** → PROMPT 봉인(저작 자기점검 주석 제거) → 봉인 출처 스냅샷(LINEAGE 요약 `sealed_sources` · §6.3) → 린트(봉인 뒤)
  ⑤ 승격 게이트(hint_finalize) → footer 바인딩 대상 확인 → 페이로드 배관 커밋(부모 = 안내 커밋 · 합성 신원 · 주입 시각 · **ref 이동 0**)
  ⑥ promotion_target 기록(evidence_publisher set-promotion-target → finalize 방출) → 대조 → 게이트 재확인
  ⑦ 봉인(§7.3 · footer v2 · 봉인 직전 원격 재조회) → 이 태그 1개 로컬 검증(봉인된 오브젝트를 풀어 서사 린트 포함)
  ⑧ 게이트(hint_push) → push(§6.2) → 원격 태그 오브젝트 SHA == 로컬
  ⑨ 카탈로그 파생(record-missing · §6.4) + 두 경로 자동 커밋  ⑩ 캠페인 셀이면 publish 위상(proof = refs/tags/<tag>@<원격 SHA>)
hint.py refacts --draft <draft>   같은 generated_utc 로 FACT 블록 · PAYLOAD · LINEAGE · PROVENANCE · template_facts 재생성(저작 산문 · tail.json ·
                                  factcheck.json 보존 · 바뀐 FACT diff 출력 · 부수효과 = draft 안뿐 · 커밋 뒤 거부)
```

- **캠페인 밖 재생**: 캠페인 밖 발행의 입력은 이미 있는 발행 기록뿐이다(발행 기록은 `evidence_publisher` 가 만든다 · docs.md §publisher
  계약). `publish --publication <topic> --replay [--node …]` 는 그 기록(`docs/_evidence/<topic>.json`)과 work-manifest 를 **읽기만**
  하고 draft 밖에 한 바이트도 쓰지 않는다(발행기 구동 ✗). 게이트 사전 확인은 읽기(판정을 stdout 에 낼 뿐)라 재생에서도 묻는다 —
  옛 기록이 승격 불가면 저작을 다 한 뒤에야 아는 것을 막는다(2026-09-22 리뷰). 재생 draft 의 `continue` 는 그 옛 발행 기록의
  promotion_target 을 새 태그로 다시 적는다(같은 기록의 새 판본 · 옛 태그는 판정하지 않는다 · P1).
- **캠페인 셀의 서사 증거**: 발행기 구동은 셀의 plan · devlog · testlog 를 요구한다. 없으면 쓰기 전에 `HINT_NARRATIVE_EVIDENCE_ABSENT`
  (셀 testlog·devlog 를 먼저 발행하고 `campaign_init.py --evidence-add` 로 등록한다) — 이것이 "문서 완료 후" 의 기계 판정이다.
- **발행 기록 시각은 불변 — 그리고 채택된다**(v7 · plan §4.8 V11①): 셀 발행 기록이 이미 다른 `generated_utc` 로 묶였고 그 발행의 draft 가 **스캐폴드되지
  않았으면**(앞선 publish 가 스캐폴드 전에 거부 · 중단됐다) publish 가 묶인 시각을 **채택**해 진행하고 로그로 알린다(명시한 시각은 쓰지 않는다 —
  사람이 거부된 publish 의 시각을 기억해 넣던 손작업이 셀마다 3회 나왔다). 스캐폴드된 draft 가 있으면 새 시각은 같은 기록의 재바인딩이므로
  막는다(`HINT_PUBLICATION_TIME_BOUND` — 그 draft 를 이어 저작한다 · 개정판은 그 발행을 끝낸 뒤 새 기록으로). draft 자리는 비어 있어야 한다
  (`HINT_DRAFT_EXISTS` · 끊긴 publish 의 도구 산출 `inputs/` 와 빈 `payload/` 만 이어 쓴다). draft 에 `payload/` 가 없으면 `HINT_DRAFT_PAYLOAD_ABSENT`.
- **FACT 재파생 = `refacts`**(v7 · plan §4.8 V11②): 생산자(증거 · 도구)를 고친 뒤 같은 `generated_utc` 로 사실만 다시 만든다 — 옛 처방(`--out` 재생성 + 손
  이식 + refresh · `git stash`)을 대신한다.
- **카탈로그 자동 커밋**(v7 · plan §4.8 V11⑤): continue 끝에서 `HINTS.md` · `hints/index.json` **두 경로만** 커밋한다(메시지 `chore(hint): 카탈로그 파생 —
  <태그> 발행 반영` · 도구 커밋이라 Co-Authored-By 줄 없음 · 훅 우회 ✗). 추적 파일에 다른 staged/unstaged 변경이 있으면 커밋하지 않고 경고한다
  (남의 변경을 도구 커밋에 섞지 않는다 · 카탈로그는 워킹트리에 남는다). 바뀐 것이 없으면 커밋하지 않는다.
- **멱등 재실행**: continue 의 각 단계는 `state.json`(도구가 쓴다 · 손 편집 ✗)에 남고 재실행은 끝난 단계를 확인만 한다. 커밋 시각은 첫
  커밋 전에 state 에 적은 값을 재실행에서도 그대로 쓴다 — 배관 커밋의 재개는 메시지·시각이 같을 때만 성립한다(새 시각 = 같은 트리의
  중복 커밋). 봉인의 재개는 같은 앵커 · 같은 annotation · 합성 tagger 일 때만(tagger 시각 무관). push 기록 시각은 그 push 를 **한**
  호출의 주입 시각이다.
- **`--no-push`** 는 봉인·로컬 검증까지 한다. 이어가기는 **continue 재실행**이다(캠페인 밖이면 같은 승인 전사를 다시 준다) — 단독
  `push --apply` 는 태그만 밀고 카탈로그 파생과 캠페인 publish 위상을 하지 않는다(그 사실을 stderr 로 말한다). 오프라인 재생은
  `--no-push --guide-commit <SHA>` 로 안내 커밋을 지정한다(원격 조회를 하지 않았다는 사실이 state 에 남는다 · push 까지 가는 발행에서는
  `HINT_GUIDE_PIN_REQUIRES_NO_PUSH`).
- **단독 `verify` · `push`**: 이 태그 1개만 본다(§6.3). 서사 린트는 그 태그를 봉인한 draft(`inputs/template_facts.json`)가 있어야 돈다 —
  draft 를 찾지 못하면 `HINT_LINT_DRAFT_ABSENT` 로 검증이 실패한다(검증 범위 = 전송 범위 · 린트를 조용히 건너뛰지 않는다). 자동으로
  `hints/.drafts/` 에서 태그로 찾고, 아니면 `--draft` 로 지정한다. `push` 는 `--apply` 가 없으면 dry-run 이고 dry-run 실패도 삼키지 않는다.
- **게이트 결과의 모양**: 게이트가 걸린 명령(`continue`→`hint_finalize` · `verify`→`hint_verify` · `push`→`hint_push`)은 첫 부수효과 전에
  `completion_gate.py authorize --mode promotion` 을 묻는다. 거부 = 게이트 JSON 원문을 stdout 에 그대로 + 그 종료코드. 게이트를 실행하지
  못했거나 출력을 읽지 못하면 같은 모양의 결정론 포장 JSON(`allowed:false`)을 stdout 에 내고 exit 2. `publish`·`lint`·`excerpt`·`name`·
  `match`·`catalog`·`refacts`·`refresh` 는 게이트가 없다(태그·브랜치를 만들지 않는다 · match 는 수신자 평면이라 게이트를 걸면 gitless 배포본에서
  죽는다). `branch-transition` 은 발행이 아니라 형식 전환이며 사람 승인 전사를 직접 요구한다(§6.5).
- **이미지 탐침 = 기본 · 읽기 전용은 선택**(2026-09-22 S2 round 3): 플래그 없는 `publish` 는 docker 를 `image inspect`·`history` 와
  **시작하지 않는** 컨테이너의 `create`(`--pull never` · `--network none` · `--entrypoint /bin/true`) · 그 컨테이너에서의 `cp` · 그 컨테이너의
  `rm` 으로만 부르고 `image_probe` 능력을 선언한다 — 측정 이미지 안 패치 사본 · 적용 표지 · 빌드 원장을 꺼내 `image-probe(script-sha+marker)` ·
  `reconstructed(fail-loud+built)`(판정한 바이트 = 이미지 바이트) 판정을 낸다(`run` · `start` · `exec` · `build` 는 거부 — 원장 cat 도 컨테이너를
  시작하므로 쓰지 않는다). 왜 기본인가: round 2 는 탐침을 옵트인(`--docker-probe`)으로 두었는데 2차 재생이 그 플래그를 쓰지 않아 태그2 셀의
  패치 9개 중 7개를 다시 `unobserved` 로 실었다(측정 이미지는 이 호스트에 있었다 · 탐침하면 1개). 정책 자체검사(runtime_selftest)의 docker
  계약도 같은 모양으로 넓혔다(`_hint_docker_calls_non_starting` — 시작 · 변경 호출 · 만들지 않은 컨테이너의 cp/rm · 지우지 않은 컨테이너 =
  위반 · 플래그 없는 publish 가 실제로 create+cp 했는지도 본다). `publish --docker-read-only` 는 탐침을 끈다 — docker 는 `image inspect`·
  `history` 뿐이고 이미지 안 원장 · 패치 사본은 탐침 기록에 관측 실패 · skipped 로 남는다(관측 실패 ≠ 원장 없음). `--docker-probe` 는 옛
  이름이다(받되 아무것도 바꾸지 않는다 · 두 플래그는 함께 쓰지 않는다). publish 출력은 탐침 결과와 미관측 수를 적고, 탐침이 돌지 않은 채
  미관측이 남으면 처방 줄(`--docker-read-only` 를 빼고 다시 · 이미지가 없으면 탐침 불가)을 낸다. `publish --no-remote-check` 는 원격 이름 충돌 조회를 건너뛴다
  (로컬만 · 오프라인 저작). 원격 조회 실패는 재생 · `--no-remote-check` · `continue --no-push` 에서만 견디고, 그 밖에서는 '조회 불가' 를
  '충돌 없음' 으로 읽지 않는다(차단).

### 6.1 승인 — 셀별 사람 Y/N 의 기록 (O6 · X3 · 2026-09-21)

발행은 **제안 전용**이다 — 무인 자동 태깅 ✗. `continue` 의 **첫 판정**이 승인이고, 승인이 없으면 어느 ref 도 움직이지 않는다.

| 발행 | 승인 기록(단일 원천) | writer |
|---|---|---|
| 캠페인 셀 | `campaign.yaml` `hint_targets[] = {node_id, cells:[명시 · 1개 이상 · 그 노드의 배정 셀], approval:{approved_by, approved_utc, source}}` — 선언 확인 팝업(기본 `declaration-popup`) 또는 셀 발행 팝업(`publish-popup`)에서 받은 사람 발화의 전사 | `campaign_init.py --hint-approve --node <n> --cells a,b --approved-by "<전사>" --utc <UTC> [--source …] [--campaign-id <id>]` |
| 캠페인 밖(재생) | `continue --approved-by "<사람 발화 전사>" --approved-utc <UTC>` | 사람 발화를 받은 Agent |

- 캠페인 셀에 `--approved-by` 를 주면 거부한다(`HINT_APPROVAL_SOURCE_CONFLICT` — 원천은 하나다). 전사는 한 줄이며 자리표시(`<<…>>`)는
  승인이 아니다(`HINT_APPROVAL_INVALID`). `approved_by` 는 **페이로드로 복사된다** — 이름·연락처가 아니라 역할과 발화 요지를 적는다.
- 사전승인이 캠페인을 셀마다 멈추지 않게 한다(2026-09-08 "진행을 막는 자리는 셋뿐" — 선언 확인 팝업이 셀별 발행 Y/N 을 겸한다).
  무인 자동 태깅 ✗ 는 그대로다: 사람이 **명시 셀 목록**을 승인한 것이다. 서브용 파생 선언(`--emit-slice`)은 `hint_targets` 를 비운다(§6.6).
- 승인된 Y/N 뒤의 **태그 push 는 에이전트 상시승인**(2026-08-24)이다. 브랜치 push 는 발행에 속하지 않는다(§6.2 · §6.5 형식 전환만).

### 6.2 push — 정확한 태그 1개 (O2 · 2026-08-20 · 2026-09-04 C-3)

- refspec 은 `refs/tags/<그 태그>:refs/tags/<그 태그>` **정확히 1개**다 — glob ✗ · `refs/heads` ✗ · `--tags` ✗(로컬의 다른 hint 태그 ·
  last-good 류 ref 유출) · 모든 push 에 `--no-follow-tags`(운영자 설정 `push.followTags=true` 가 옛 미검증 태그를 얹어 보낸 것을 격리
  저장소에서 재현 · 2026-09-22). 성공 로그는 **실제 refspec** 을 출력한다.
- **hint 브랜치는 발행이 밀지 않는다.** 태그 push 가 페이로드 커밋 오브젝트(와 그 부모 = 안내 커밋)를 함께 나르므로 zip 은 태그만으로
  성립하고, 원격 hint 브랜치 tip(= 안내 커밋)은 발행으로 움직이지 않는다. 브랜치가 움직이는 것은 페이로드 **형식이 바뀔 때**
  `hint.py branch-transition`(사람 승인 전사 · exact refspec `refs/heads/hint:refs/heads/hint` · non-force)뿐이다(§6.5 · U8 — 발행기에 브랜치
  자동 push 권한 ✗).
- push 전에 이 태그 1개를 로컬 검증하고(검증 범위 = 전송 범위), 원격에 같은 오브젝트가 이미 있으면 push 를 생략(멱등), 다른 오브젝트면
  `HINT_REMOTE_TAG_CONFLICT`(강제 ✗). push 뒤 원격 태그 오브젝트 SHA 를 로컬과 대조한다.
- **자격증명**(2026-09-04 "could not read Username" 사건): https 원격만 토큰이 필요하다(환경변수 `GITHUB_TOKEN` → `envs/.env` 의 같은 키 ·
  없으면 `HINT_PUSH_CREDENTIAL_ABSENT`). ssh · scp형 · 로컬 경로 원격은 토큰 불요. 토큰은 argv · URL · reflog · remote.url 에 남지 않고
  (`credential.helper=` 로 상속 헬퍼를 비운다 · `GIT_TERMINAL_PROMPT=0`), 평문 http · URL 속 자격증명은 거부한다. 같은 경로를 upstream
  `push_branches.py` 가 공개 API `hintlib.tag.git_push_authenticated` 로 재사용한다.

### 6.3 게이트 범위 = 이번 발행분만 (D10 · P1 의 귀결)

`verify` 는 push **전** 로컬 봉인 검증이며 **이 태그 1개**만 본다: annotation footer 파싱(v1 · v2) · 대상 = 부모가 안내 커밋인 페이로드 커밋 ·
트리 allowlist · PII(annotation · 트리 · 파일 이름 · tagger · 커밋 신원) · `PROVENANCE.tag` = `PAYLOAD.tag` = 태그 · PROMPT 잔존 0 · 이름 재조립
일치 · 봉인된 오브젝트에서 서사 린트. 카탈로그 등재 확인은 push **후** 파생이 한다.

**봉인 뒤 재현성**(v7 · plan §4.9 · V5): v6 DS4F 태그는 봉인 13초 뒤 출처 devlog 가 정정되자 `verify` 가 FAIL(`HINT_SIGNATURE_MISMATCH`)로 바뀌었다 —
린터가 가변 비추적 docs 를 읽었기 때문이다. 이제 봉인 때 발췌 · 서명 · 벤치 절 출처 파일의 sha256 을 LINEAGE 요약 `sealed_sources[{path,
sha256}]` 에 싣고(비추적 docs 는 git 이 바이트를 들지 않으므로 `policy:GIT_SINGLE_AUTHORITY` 2문항상 **맹점층** — 기록 정당), 봉인 **후** verify 는
출처 의존 린트 코드(발췌 · 서명 불일치 등 `tag.SOURCE_DEPENDENT_LINT_CODES`)를 그 스냅샷과 대조해 출처가 바뀌었으면 FAIL 이 아니라
`INFO: 출처 변경됨(봉인 뒤)` 로 보고한다. 구조 검사(footer · 트리 allowlist · PII · 앵커 · 신원 · 스냅샷 모양)는 그대로 FAIL 이고, 스냅샷이 없는
v6 이하 태그와 출처가 그대로인 발견은 강등하지 않는다. 봉인 **전** 린트는 fail-closed 그대로다. 과거 · 타 PC 태그의 결함은 신규 발행을 막지 않는다 — 옛
verify 는 로컬 태그 **전수**를 원격 파생 index 와 대조해 이 체크아웃에서 구조적 RED 였다(F12 · `MANIFEST_REF_ABSENT` 18 ·
`ANCHOR_MISMATCH` 2). 원 선례는 §11 의 2026-07-31(신규 1개가 레거시 23개의 형식 미비로 영구 차단)과 2026-08-20(드리프트 16건이 신규
7건을 막았다)이다.

### 6.4 카탈로그 — 원격 발행 태그에서 파생한다

- 진실원천은 `git ls-remote` 다(plan_26090107 D1.1). 손저작 색인 경로(`index`·`reindex`)는 폐쇄됐다 — 그것이 감사 ④⑤(격리 세탁 ·
  오염 brief 가 18행을 삼킴)의 기전이었다. 원격 조회 실패 = 캐시로 대체하지 않고 **중단**한다(낡은 카탈로그는 부재와 구분되지 않는다).
- `hints/index.json`(schema 4)과 `HINTS.md` 는 **전량 생성물**이다 — 머리말 산문까지 파생한다(손산문 ✗ · 옛 손산문은 사라진 태그 · 없는
  열 · 존재하지 않는 3세그먼트 사용법을 들고 낡았다). 열: 태그 · 문법(v7 · v6 · 옛 세대) · vLLM · 모델 · arch · **판정**(v7 · 태그 커밋의
  `PAYLOAD.measurement.verdict` · 없으면 `—` · v6 도 읽을 수 있으면 표시 · 재판정 ✗) · bench_mode · 결손 · brief. 원격이 광고한
  태그 오브젝트 SHA 를 직접 읽는다(K5 — 이름만 남기면 같은 이름의 다른 로컬 오브젝트 brief 가 붙는다).
- 원격에 있는데 로컬 태그 오브젝트가 없으면 brief 를 지어낼 수 없다(합성 금지). 기본 모드는 중단 + 종료코드 4 + 조회한 그 원격의 fetch
  안내(자동 fetch ✗ · 2026-09-14). **`--record-missing`**(X15 · D-i)은 그 태그를 `object: absent-local` 행(brief `—` · 결손 `미수령`)으로,
  annotated 가 아닌 원격 태그를 `object: not-annotated` 행으로 **정직하게** 싣고 rc 0 — 타 PC 발행 1건이 카탈로그 갱신 전체를 막던 것을
  D10 과 합성 금지를 둘 다 지키며 푼다. `continue`(push 뒤)와 형식 전환 뒤 재생성은 이 모드를 쓴다. continue 는 파생 뒤 두 경로를 자동
  커밋한다(§6 · 다른 추적 변경이 있으면 커밋 ✗ + 경고).
- 갱신 권한은 **중앙 권위 선언**(`hints/.central_authority` · 비추적)이 있는 체크아웃만 갖는다(`HINT_CATALOG_CENTRAL_FLAG_ABSENT`). 선언은
  자격증명이 아니라 역할 선언이며, 각 저장소가 자기 원격에 대해 스스로 한다(배포본에 실려 복사되면 안 된다). 옛 D8 비대칭
  (2026-08-20 `plan_26082009` — "발행(봉인)은 분산, 색인·**배포**는 중앙")에서 이 선언은 태그 push 도 막았다. 선언이 없는 체크아웃은
  **push 하지 않는다** — `continue --no-push` 로 로컬 봉인까지만 만들고 태그를 상류에 전달한다. 도구가 집행한다: 선언이 없으면
  push 까지 가는 `continue`(승인 확인 직후 · 커밋·봉인 전)와 단독 `push` 가 `HINT_PUSH_CENTRAL_AUTHORITY_ABSENT` 로 멈춘다(부수효과 0 ·
  2026-09-22 통합에서 옛 hint_tag push 게이트 복원).
- 수신자 검색 `hint.py match --vllm V --model M [--arch A]` 는 읽기 전용 · gitless(`hints/index.json` 만)이며 모델 관계 = **정규화 슬러그
  동치 + v6 · v7 태그의 `base_model`**(PAYLOAD.identity 파생)이다. 결정론부로 1차 매칭하고 꼬리 뜻은 보조 표시다. 패턴/안티패턴 판정은 하지
  않는다 — 시간축은 수신자만 본다. family 색인
  (옛 `families.json`)은 폐기됐다(O3 · "파생 가능한데 손으로 적은 것").

### 6.5 태그는 안내 커밋을 부모로 한 페이로드 커밋을 가리킨다 (v7 · 2026-09-29 · plan_26092908 §4.7 · U8 · V12)

태그는 소스 트리 커밋이 아니라 **페이로드 커밋**을 가리킨다 — 그래서 zip 이 곧 재현 키트이고 `.claude/` 같은 프로젝트 소스가 딸려 가지 않는다
(옛 계약은 이 규칙을 적어 두고 검사하는 코드가 없어 native 태그 3종이 소스 트리 커밋에 봉인된 채 나갔다 · 2026-09-07).

**v7 브랜치 모델**: v6 까지 페이로드 커밋의 부모는 **로컬** hint tip 이었다(브랜치 한 칸 전진) — PC 마다 체인이 갈라졌고, 다른 PC 태그가 이 PC
verify 에서 막혔고, 브랜치를 push 하면 원격 첫 화면이 마지막 셀 README 가 됐다. 이제:

- **안내 커밋** = 트리가 `README.md` 하나(100644)이고 README **첫 줄**이 형식 마커 `<!-- hint-branch-format: v7 -->` 인 커밋 — 원격
  `refs/heads/hint` tip 이다. 마커는 발행기가 "이 브랜치가 어느 형식의 안내인가" 를 읽는 유일한 기계 표면이다(산문으로 판정 ✗). 안내 README 의
  정본은 템플릿 `templates/hint-branch-README.md` 다.
- **페이로드 커밋의 부모 = 안내 커밋**(continue 가 원격 tip 을 `ls-remote` 로 **읽기만** 한다 · 오브젝트가 없으면 그 한 ref 만 fetch · 로컬 브랜치
  이동 ✗). 페이로드 커밋끼리 체인을 만들지 않고 브랜치에 얹지도 않는다 — **태그만** 자기 커밋을 가리킨다 → PC 간 분기가 원천적으로 없다.
  재개 멱등성은 draft state 의 anchor 와 결정론 SHA(같은 페이로드 · 메시지 · 시각 · 안내 = 같은 커밋)로 판단한다.
- **형식 검사**: 원격 안내의 형식 ≠ 발행기 형식이면 `HINT_BRANCH_FORMAT_MISMATCH` — **마커 없는 옛 안내**(v6 전환 커밋처럼 README 하나지만 마커
  도입 전)도 같다(형식을 선언하지 않은 안내를 v7 로 추정하지 않는다). 원격 조회 실패 = fail-closed.
- **봉인 · 검증이 셋을 강제한다**(저장소 안 git 객체만 · 네트워크 ✗): 앵커의 부모가 정확히 하나이고 형식 마커를 가진 안내 커밋인가
  (`HINT_ANCHOR_PARENT_NOT_GUIDE` — 형식 버전은 묻지 않는다: 뒤 형식 전환 뒤에도 앞 형식 안내 위의 태그는 유효하다) · 앵커 트리에 `PAYLOAD.json`
  (`HINT_ANCHOR_PAYLOAD_ABSENT`) · `PROVENANCE.tag` = 이 태그(`HINT_ANCHOR_PROVENANCE_TAG_MISMATCH`). 마커 도입 전 체인형 태그는 판정 대상이
  아니다(D10 · P1). 검사 범위는 이 태그 1개다(§6.3).
- **형식 전환**(`hint.py branch-transition --remote R --generated-utc U --approved-by "<전사>" --approved-utc U`): 원격 tip 위에 이 발행기 형식의
  안내 커밋을 얹고 exact refspec 로 non-force push 한다 — **브랜치 push 의 유일한 경로**이고 사람 승인이 필수다(없으면 `HINT_APPROVAL_ABSENT` ·
  ref 쓰기 0). 원격이 이미 이 형식이면 `already-transitioned`(로컬 hint 가 다르면 원격 tip 으로 맞춤 = 옛 체인형 로컬 정리) · 원격이 더 새
  형식이면 `HINT_BRANCH_TRANSITION_DOWNGRADE` · 로컬 hint 를 옮기면 태그가 붙잡지 않는 페이로드 커밋이 고아가 되면
  `HINT_BRANCH_TRANSITION_ORPHAN_PAYLOAD`(아무것도 쓰지 않는다). 태그 · 카탈로그 · 코드 워크트리는 건드리지 않는다.
- 페이로드 커밋은 **배관만** 쓴다(임시 인덱스 · commit-tree). 메인 워킹트리 · 인덱스 · HEAD · 모든 ref 는 움직이지 않는다. **hint 브랜치를
  체크아웃하지 않는다** — 브랜치 전환이 산출물을 파괴한 실측 이력이 있다. hint 브랜치는 빌딩블럭이 아니라 산출물이므로 `sync_branches`
  대상이 아니다.
- 매 발행 트리를 allowlist 로 **새로 짓는다** — N 번째 archive 에 이전 태그의 파일이 딸려 오지 않는다(이력은 커밋·태그로 남는다).
  브랜치 tip 의 README 를 페이로드에 심던 옛 동작은 폐지됐다(K8 · 페이로드 README 는 형식 버전으로 생성한다).

### 6.6 main-only (정책 C4)

발행은 **메인 단독**이다 — references.md 와 같은 빌딩블럭 전파 평면이며 서브 egress 상태와 무관하다. 발행 엔진(`hint.py` · `hintlib`)은
원격 셸을 실행하지 않고 서브 배달·릴레이 도구를 부르지 않는다. 메인→서브 배달은 엔진도 `hints/` 트리도 나르지 않고, 서브의 런타임 스킬
평면에 hint-publisher 가 없으며, 서브용 파생 캠페인 선언은 `hint_targets` 를 싣지 않는다. 서브 산출물은 문서기반 회수 뒤 메인이 재저작한다
(`.claude/rules/docs.md` 상향 회수 규약 · 코드·설정 직접 회수 금지).

### 6.7 발행 전 독립 사실 검증 (2026-09-22 · plan_26092119 S2 round 2 F11)

**왜**: S2 오프라인 재생(태그2 셀)의 1차 페이로드는 린트 0 · 발췌 무결성 통과였는데 독립 검증에서 사실 오류 **25건**(오답 5 · 오도 16 · 근거
없음 4 · 약 460 주장 중)을 냈다 — 발췌 무결성은 "인용이 원문과 같은가" 만 보고 "주장이 원문과 맞는가" 는 보지 못한다. 틀린 1건(#28 "두 번
띄웠는지 미기록")은 같은 페이로드가 인용한 원장에 선언 2건으로 적혀 있었다. 그래서 린트 다음에 **저작자가 아닌 검증자**를 둔다
(헌법 불변식 B: 누락은 기계가, 거짓은 리뷰가 — 이 단계가 그 리뷰를 발행 절차 안에 고정한다).

**절차**: 저작 → lint 0 → **다른 Agent**(저작자 ≠ 검증자)가 00~03 산문 · hint-event · **FACT 블록**의 사실 주장을 전부 뽑아 인용 출처(계보 문서 ·
블랙박스 원장 · 엔진 로그 · docker history · 이미지 안 파일 · 인증서)와 대조 → `<draft>/inputs/factcheck.json` → 지적마다 고친다 — 산문
(`target: prose`)은 저작자가 고치고(또는 주장을 빼고), FACT(`target: fact`)는 편집하지 않고 **지적만** 한 뒤 생산자(증거 · 도구)를 고치고
`hint.py refacts` 로 재파생한다 → 항목을 `status: fixed` + `resolution` 으로 닫는다 → lint 0 → continue.

**왜 FACT 도**(v7 · 2026-09-29 · plan §4.5 V4): v6 의 "기계 전사는 오기 0" 전제가 깨졌다 — D1 FACT 가 09-12 옛 원시를 이번 측정으로 읽었고, N1
FACT:reproduce 에 호스트 경로가 실렸고, D2 · N1 에서 기록된 값을 "미관측" 이라 적어 산문 오류를 유도했다(사실검증 부류 7 이 D2 8/13 · N1 3/7).
사실 검증이 FACT 를 보지 않았으므로 그 오류가 봉인됐다. 부류 7 은 FACT 도 대상이다.

**점검 7부류**(1차 오류 25건의 부류 · 정본 상수 `template.FACTCHECK_CLASSES` · 템플릿 4종 최상단 `<!-- SELFCHECK -->` 저작 자기점검과 같은 목록):

| # | 부류 | 1차 사례 |
|---|---|---|
| 1 | 수치의 귀속(어느 셀 · 실행 · 측정) | 다른 셀의 재현 밴드 · 자매 셀 overhead 를 이 셀 값처럼 |
| 2 | 선언 대 측정 | 예산 산식의 weights 38,852MiB(선언)를 측정처럼 — 엔진 로그 실측은 39.05GiB |
| 3 | 가설 · 강등 기전 · 조건이 다른 검증을 처방으로 | MTP off 등가 PASS 를 MTP on 셀의 것으로 · 측정되지 않은 offload 무효 기전을 기각 사유로 |
| 4 | 문서 순서 · 정체 | 넷째 plan 을 "둘째 plan" 으로 · 다른 스모크의 attestation 을 "이 셀의" 것으로 |
| 5 | 출처 없는 인과 | "공식 레시피의 triton 을 옮긴 것이 원인" — 출처 없음 |
| 6 | '유일한 차이' 주장 | "KV dtype 만 다름" — 인증서 소프트 지문은 moe_backend 도 달랐다 |
| 7 | 기록된 것을 미관측이라 함 | 원장의 선언 2건 · docker history 의 가드 문구 · 이미지 안 vLLM HEAD 를 "미관측" 으로 |

**보고 형식**(`inputs/factcheck.json` · schema 1): `{schema_version: 1, tag, author, checker, checked_utc, claims_checked ≥ 1,
classes_checked: [1..7] 전부, items: [{id, where, claim, verdict: wrong|misleading|unsupported, class: 0..7, truth, source,
status: open|fixed|disputed, resolution, target: prose|fact}]}`(`target` 필수 — 누가 고치는지를 보고서가 말하지 않으면 FACT 오류가 산문으로
덮인다). author · checker 는 **역할 이름**이다(00 메타에 실린다 · 사람 이름 ✗).

**게이트**(hint.py continue · 린트 0 뒤 · 봉인·커밋 전 · 부수효과 = draft 안의 00 메타 · PAYLOAD 요약뿐):

- 부재 → `HINT_FACTCHECK_ABSENT` · 모양 결함(검증자 = 저작자 · 7부류 누락 · 다른 draft 의 태그 · fixed 인데 resolution 없음 · 어휘 밖)
  → `HINT_FACTCHECK_INVALID` · 판정 항목 중 `fixed` 가 아닌 것(`open` · `disputed`) → `HINT_FACTCHECK_OPEN`.
- `unsupported`(근거 없음)도 막는다 — 요구("열린 오답·오도")보다 **의도적으로 넓다**: 출처 없는 단정은 수신자에게 오도와 같은 피해를 내고,
  1차에서 부류 5 가 이 판정으로 나왔다.
- 탈출구는 사람 결정 하나 — `--factcheck-waiver "<사람 발화 전사>"`(한 줄 · 자리표시 ✗ · 모양 결함 = `HINT_FACTCHECK_WAIVER_INVALID` 로
  쓰기 전에 거부). 면제는 state.json · 00 메타("사람 면제") · `PAYLOAD.factcheck.status = waived` 에 남는다 — 수신자가 사실 검증 게이트를
  통과하지 않은 발행임을 안다(무엇을 면제했는지 = `reason_code` · 00 메타가 그 사유대로 "보고 없이" · "모양 결함인 채" · "지적이 열린 채" 로
  적는다 · 2026-09-22 통합 정정: 옛 문구 "독립 검증 없는 발행" 은 열린 지적 면제에서 사실이 아니었다). 도구는 보고의 **모양과 열린 항목만** 판정한다(검증의 질은 사람 Y/N 이다 · "기계는 빈칸, 사람은 쓸모").

## 7. 페이로드 형식 (`format: hint-payload/v7`)

### 7.1 트리 (닫힌 allowlist)

```
README.md              수신자 안내 — templates/payload-README.md 를 형식 버전으로 렌더(정적 복사 ✗) + PAYLOAD.naming 에서 생성한
                       "이 태그 이름 읽는 법"(세그먼트별 값 · 뜻 · 출처 · 꼬리 토큰 뜻 · timestamp · native/-bare · 세대 표) · 매핑 표 요약
00-hint.md             지도 — 0.1 요약 · 0.2 유효맥락 · 0.3 벽 지도 요약 · 0.4 결정론 해소값 · 0.5 서빙 노브와 값의 지위 ·
                       0.6 재검증·라우팅 · 0.7 메타 · 0.8 comment(자유) · 0.9 이름 꼬리   ← 수신 Agent 가 가장 먼저 읽는다
01-artifacts.md        1.1 적용 판정 표(검증 · 관련성 열) · 1.2 슬롯별 적용 사유 · 1.3 값의 지위표 · 1.4 재현 절차(슬롯 → 빌드 컨텍스트 매핑) ·
                       1.5 (multi) 서브 레시피 해설
02-narrative.md        계보 서사 — 2.1 출발점 · 2.2 벽과 해소 · 2.3 기각된 시도·반증된 축 · 2.4 오진과 정정 · 2.5 값의 이력 ·
                       2.6 되풀이하지 말 것 · 2.7 열린 물음
03-benchmark.md        3.1 측정 결과(판정 · §3.0.2) · 3.2 부하 곡선(기계 렌더) · 3.3 측정 구성 · 3.4 측정 명령 원문 · 3.5 like-with-like
PAYLOAD.json           기계 사실(§7.2)
LINEAGE.json           수신자 요약(문서 stem · 역할 · 날짜 · 발췌 수 · ≤ 20KB) + 봉인 출처 스냅샷 sealed_sources(§8.1 · §6.3)
PROVENANCE.json        {schema_version:2, tag, source_anchor, source_anchor_is_head, assembly_branch, payload_files[], generated_utc}
artifacts/<slot>/…     triplet · runtime_patch · build_patch_pre · build_patch_post · build_recipe · compose · fork_pin — 적용된 것만(§3 A) ·
                       Docker · native 같은 슬롯 · 파일마다 검증 표시 · 관련성(§3.3)
artifacts/compose/sub_recipe.json   (multi 전용)
```

allowlist 밖 파일 · 심볼릭 링크 · `artifacts/` 안의 `.gitattributes`/`.gitmodules`(git archive 가 따라가 zip 을 바꾼다)는 거부한다.
`slots.declaration.json` 은 폐지됐다(01 의 적용 사유와 중복 → `PAYLOAD.slots` 흡수 · 대사 규칙은 유지). 챕터 목록은 템플릿
(`templates/*.prompt.md`)과 `template.CHAPTERS` tripwire 가 정본이다.

### 7.2 PAYLOAD.json (schema_version 3 · v7)

키: `schema_version` · `format` · `tag` · `generated_utc` · `campaign{id, cell, node, mode}` · `identity{model, gpu, vllm(엔진 자기보고), quant,
topology, tp, hf_repo, base_model, base_slug, quant_composition[{scope, dtype, source}], source}` · `naming{grammar: "v7", segments, axes,
tail[{token, meaning, evidence{file, key, value}}], timestamp, vllm_build_input, vllm_observed}` · `plane` ·
`build{track, dockerfile, image_tag, image_digest, vllm_repo, vllm_ref, vllm_sha, torch, cuda, ngc, cpu_arch, driver, driver_by_node,
driver_conflict{observed, declared, sources}, os, source}` · `applied_set{status,
source, patches[], reconstruction, probes}` · `slots{<slot>:{files, file_records[{path, sha256, verification, generated_by, verification_basis, relevance,
relevance_basis, header(생성 파일만)}], applicable, rationale, evidence, confidence}}` · (v7) `agent_requests[]` · `build_context_map` · `missing_reasons` ·
`native_source` · `qualification{health_200,
inference_observed, sources, method}` · `measurement`(§3.0.2 — `verdict` · 판정 수치 · `sources{키: 출처}`) · `measurement_config`(§3.0.1 D-f) · `missing[]` · `approval{approved_by, approved_utc,
source}` · `evidence_pointers[]` · `publication{topic, manifest_ref, task_class}` · `bench_definition` · `factcheck` · (2026-09-22 S2 round 3)
`measurement_env_observed[{key, value, node, source, kind, occurrences}]` · `tool_snapshots[{name, repo_path, git_rev, snapshot_rel, …}]` ·
`attestation_scope{config, written_utc, phase, bound_by, path, scope}|null` — 00~03 사실 블록과 같은 값(Agent 표면).

- **값 옆에 출처**를 둔다(`source` · `*_source` — 결정론 규율: 측정·모의·공식은 데이터에서 구분돼야 한다). 값은 관측이 정본이다(F4:
  사람이 적은 "NGC 26.05" 는 서빙 이미지 실측과 달랐다) — 0.4 결정론 해소값은 이미지 inspect · 빌드 원장에서 기계가 채운다.
- **관측 > 선언**(v7 · plan §4.5 V3): 드라이버 · 커널 · CUDA 같은 호스트 사실의 우선순위는 attestation parity(노드별 관측) > 스윕 meta > 인증서 >
  manifest 다. 스윕 meta · 인증서가 manifest 선언을 옮긴 값이면(관측 표지 없음) 출처에 `(manifest 선언 옮김)` 을 붙이고, 관측과 선언이 다르면
  `build.driver_conflict` 에 **둘 다** 싣는다(v6 는 DS4F · D1 · N1 에 manifest 의 580.173.02 를 "측정" 으로 봉인했고 같은 zip 의 attestation 은
  실측 580.178.04 를 들고 있었다).
- **"미관측" 오판 제거**(v7): 이미 관측 가능한 값(빌드 원장 `requirements_sha256` ↔ 실린 requirements · `script_sha256` · image digest · 시각)을
  "미관측" 으로 내는 경로는 자체검사 음성대조로 막는다 — 기계가 빈칸을 내면 저작자가 그 빈칸을 사실처럼 옮긴다(부류 7).
- **절대경로 금지**. 경로는 저장소 상대 또는 `<manifest.<field>>` · `<repo>` 치환. 저장소 밖 포인터는 `<outside-repo>`.
- **env 형상**: 토폴로지 `.env` 실물은 배포하지 않는다(운영자 NAS 루트 · 호스트 · 계정을 담는다). **키를 전부 남기고 값만 치환**한
  형상 템플릿을 싣는다(2026-09-06 Q9: 키를 지우면 수신자는 그 변수의 존재 자체를 모른다 — 멀티 클러스터 5변수가 그렇게 빠졌다).
  `.env.cluster`·`.env.interconnect` 는 upstream `render_dockerfile.env_tier(key)` 3층으로, 그 밖의 `.env` 는 키의 신원성 규칙으로 치환한다.
- **서브 레시피**(multi · D3): 서빙 평면에서 메인 대비 달라지는 것은 Ray worker 역할뿐이다(같은 이미지 레시피 · 같은 compose · 같은
  `serve_runner.sh` 가 `NODE_ROLE` 로 분기). `sub_recipe.json` 은 role_diff · launch_order · env_forward · image_identity_args ·
  per_node_values(자리표시) · preconditions · parity_attestation 을 싣고, 서브 전달 목록은 upstream `slave_forward.py` 가 **유일하게**
  파생한다(스모크와 hint 가 같은 함수 · 2026-07-24~09-09 다섯 번의 침묵 누락). 서브를 다시 스캔하지 않는다 — 스모크가 회수한
  attestation 만 읽는다.

### 7.3 annotated 태그 (D4)

```
<brief — 00-hint.md §0.1 첫 문단(최대 3줄) · 기계 추출>

전체 지도·서사·재현 키트는 이 태그의 zip(archive) 안에 있다 — `00-hint.md` 부터 읽는다.

<!-- hint-evidence-binding:v2 · 주소는 발행 저장소 로컬 경로(수신자 해소 대상 아님)
version: 2
tag: hint/…
topology: <topology> TP=<tp>[(Ray)]
anchor: <페이로드 커밋 40자>
manifest_ref: docs/_evidence/<topic>.work-manifest.json
bench_ref: <인증서 또는 bench_report 상대경로>
bench_kind: certificate | bench_report
-->
```

- annotation 은 **정확히** brief + 포인터 + footer 다 — 그 밖의 바이트는 거부한다(`HINT_ANNOTATION_SHAPE`). brief 를 손으로 쓰면 zip 과
  다른 말을 할 수 있으므로 00 §0.1 추출과 다르면 거부한다(`HINT_ANNOTATION_BRIEF_MISMATCH`). brief 앞에 주석·인용·표를 두지 않는다
  (감사 ⑤: 맨 위 경고 주석이 brief 로 캐내져 카탈로그 18행을 삼켰다).
- footer **v2** = **증거 주소 7필드**(version · tag · topology · anchor · manifest_ref · bench_ref · bench_kind · 이 순서 · `tag.FOOTER_FIELDS`).
  v1 의 `certificate_ref` 는 REFUTE · lite 셀에서 bench_report 를 가리키는 거짓 이름이었다(V2) — v2 는 `bench_ref` 로 주소를, `bench_kind`
  (`certificate` | `bench_report` · 닫힌 어휘 `tag.BENCH_KINDS`)로 종류를 말하고, 종류가 파일 이름(`benchmark_*.yaml` | `bench_report_*.md`)에서
  파생되는 값과 다르면 거부한다(`HINT_EVIDENCE_BINDING_MALFORMED`). **신규 봉인은 v2 만**이다(`HINT_EVIDENCE_BINDING_VERSION`) — v1 은 옛 태그
  읽기 전용이다(판 무관 읽기 = `tag.bench_binding`: v1 은 이름에서 종류를 파생 · 판정 불가면 `underivable`). 주소는 발행 저장소의 로컬 경로이며
  수신자가 해소하는 대상이 아니다(여는 줄이 그렇게 말한다). 내용 digest 3종은 폐지됐다(plan_26090222 F-6a — 무결성은 git 의 일이고 앵커는
  git 이 해시하는 커밋이다 · 은퇴 키가 보이면 거부). topology 라벨 = 파생 `"<topology> TP=<tp>"` + multi 면 `"(Ray)"`(X7).
- 태그 생성은 메시지를 stdin 으로 흘리고 `--cleanup=verbatim` 을 쓴다(2026-08-20: git 기본 cleanup 이 `## ` 줄을 전부 지웠다 — "49/49 태그
  헤딩 0개" 가 처음엔 저자 탓으로 오인됐다). 생성 뒤 오브젝트 바이트를 기대값과 대조하고 어긋나면 방금 만든 태그를 되돌린다.

### 7.4 신원 · 시각

- 페이로드 커밋의 author · committer 와 tagger 는 모두 **프로젝트 합성 신원**(`hintlib.core.SYNTHETIC_NAME`/`SYNTHETIC_EMAIL` · 예약 TLD
  `.invalid`)이다. 판정은 PII 목록 대조가 아니라 **동일성**이다. 실명 override 경로는 없다 — 옛 기본값(`git config`)은 잊으면 발행자 실명이
  배포 태그에 박히는 구조였고 tagger 에서 1회(push 전 회수), 커밋 층에서는 57회 일어났다(K1).
- 커밋 · 봉인 시각은 **주입**(`--generated-utc`)이다 — 벽시계 ✗(K2: 옛 커밋 시각은 벽시계라 같은 입력이 다른 SHA 를 냈다). 커밋 메시지는
  파생한다(`hint: <tag>` · source_anchor · payload_format · generated_utc — 발행자 자유 서술 ✗).

## 8. 서사 — 계보를 읽고 챕터별 기재 지시를 채운다 (D1 · D2)

결정론의 자리를 명시한다: 산문 자체는 결정론이 아니다. 결정론은 ① **무엇을 읽는가**(LINEAGE) ② **무엇을 적어야 하는가**(템플릿 PROMPT)
③ **기계가 채우는 사실 블록** ④ **적었는가 · 인용이 원문과 일치하는가**의 검사에 둔다. 질은 발행 시 사람 Y/N 이다("기계는 빈칸, 사람은
쓸모"). 교훈: **템플릿만으로는 아무것도 보장되지 않는다 — 집행되는 검사만 지켜진다**(2026-08-20 실측: 템플릿이 처음부터 있었는데 49/49
태그에 헤딩 0개 · 밀도 12배 차 · 집행되던 유일한 검사만 100% 지켜졌다). 그래서 모든 지시는 린트 규칙과 짝이다.

### 8.1 계보 — `LINEAGE.json` 요약 · `LINEAGE.full.json` 전체 (X1 · X17 · v7 V13)

- **분리**(v7 · plan §4.6): v6 의 `LINEAGE.json` 은 zip 최대 파일(82~160KB)이었는데 블라인드 수신자 3/3 이 무용하다고 판정했다. 이제 zip 에는
  **수신자 요약**(`kind: receiver-summary` · 문서 stem · 역할 · 날짜 · 발췌 수 · ≤ 20KB — 넘으면 `HINT_LINEAGE_SUMMARY_TOO_LARGE`)과 봉인 출처
  스냅샷(`sealed_sources` · §6.3)만 싣고, **전체**(해시 · 후보 로그 · 모호 해소 기록)는 발행자 평면인 draft `inputs/LINEAGE.full.json` 에 둔다.
  린트 · excerpt 는 전체를 읽는다. 아래 규칙은 전체 계보의 규칙이다.

- 발행 시점 파일에서 mention 그래프를 **직접** 계산한다(wiki-desk registry 비의존 — 라이브 registry 로는 태그2 계보 표적 12문서 중 2개만
  잡혔다). 입력 root = `docs/{plan,devlog,testlog,report,benchmark,simlog}`(F16: report·benchmark 가 서가 밖이었다).
- 시드 = 이 발행 기록 + 같은 identity · 같은 셀의 과거 발행 기록 + 캠페인 셀 포인터 + 선언. 조상 방향 mention + "채택된 testlog 를 서술한
  devlog" 보강 · 깊이 4 · 기반 슬러그 필터(한 홉 경유 `transit[]` 허용) · **발행 시각 상한** · `seed/`·`sync_staging/`·`.claude/`·`CLAUDE.md`
  배제(헌법 비색인 · 백업 사본의 섀도잉).
- 간선이 끊긴 계보는 사람이 `publish --lineage-add <path>=<사유>` 로 보충하고 `source: declared` 로 표시한다(사유 없는 추가 ✗ · v6 DS4F 처럼
  스스로 "계보 밖" 이라며 9회 인용한 devlog 가 빠진 좁은 계보의 처방도 이것이다).
- 엔진·빌드 로그는 문서가 아니라 원시 증거이므로 `evidence_candidates[]` 로 따로 적고 발췌 원천으로만 쓴다. 문서 0건 = `HINT_LINEAGE_EMPTY`.
- 추적 `docs/report/*` 는 경로 + 커밋, 비추적 문서는 sha256 을 싣는다(`policy:GIT_SINGLE_AUTHORITY` 2문항).

### 8.2 PROMPT 템플릿과 hint-event

템플릿 `templates/{00-hint,01-artifacts,02-narrative,03-benchmark}.prompt.md` 의 각 챕터는 세 층이다: 챕터 제목 · `<!-- PROMPT … -->` 기재 지시
(질문 · 읽을 것 · 기재 · 필수 필드 · 금지 · 블록 · 필수 발췌 · 조건 · 선택) · `<!-- FACT:<id> -->` 기계 사실 블록. 지시는 **페이로드가 아니라
저장소의 템플릿에서 읽는다** — 저작자가 지시를 지워 규칙을 벗어나지 못한다(`HINT_PROMPT_TAMPERED`). 봉인은 PROMPT 블록을 `> 이 절이 답하는
질문: <질문>` 한 줄로 바꾼다 — 지시문은 배포되지 않되 의도는 남는다. 미저작 자리표시 `<<AGENT: …>>` 가 남으면 봉인하지 않는다.
템플릿 최상단의 `<!-- SELFCHECK … -->` 는 **저작 자기점검**(§6.7 의 7부류)이다 — 봉인이 통째로 지우고(질문 줄도 남기지 않는다) 봉인 뒤
잔존은 `HINT_SELFCHECK_RESIDUE` 다(2026-09-22 S2 round 2).

**사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다**(같은 사실의 두 표면). 제한 YAML: 한 줄 `키: 값` · 리스트는 `[a, b]` 인라인만 ·
`#` 뒤 주석. PROMPT 주석 안의 형식 예시는 사건이 아니다.

| kind | id 접두 | 필수 필드 |
|---|---|---|
| `wall` | `W` | id · kind · 증상 · 서명 · 원인 · 해소 · 검증 · 전이등급 · 출처 |
| `misdiagnosis` | `M` | id · kind · 증상 · 서명 · 원인 · 해소 · 검증 · 전이등급 · 출처 |
| `rejected` | `R` | id · kind · 시도 · 기각사유 · 출처 |
| `value-status` | `V` | id · kind · 노브 · 값 · 지위 · 근거 · 출처 |
| `open-question` | `Q` | id · kind · 물음 · 현재상태 · 출처 |

- 전이등급 ∈ `arch-invariant`(그대로 참조) · `arch-scaled`(네 HW 에서 다시 잰다) · `arch-locked`(복사 금지 · 재도출) · `judgment`(맥락 재해석).
- 지위 ∈ `tuned` · `inherited`(이 셀에서 조정된 적 없음 · 손레버 포함) · `negative-control`(비교를 위해 일부러 과잉·과소) · `engine-default` ·
  `declared-requirement`(빼거나 바꾸면 실패가 관측된 필요조건 — 근거 = 그 실패의 W id **또는** 실패를 기록한 artifacts/ 파일 헤더 · §8.4).
  현재지위(오진) ∈ `확증` · `반증` · `가설(강등)` · `미결` — **원래 주장**(서명에 옮긴 당시 주장)의 지위다(2026-09-22 S2 round 3 · 정본
  `template.CURRENT_STATUS_MEANINGS`): 반증 = 원래 주장이 거짓으로 확인됐다(정정된 이해는 `해소`) · 확증 = 원래 주장이 참으로 확인됐다 ·
  가설(강등) = 원래 주장(기전)이 확인되지 않아 가설로 내려왔다(처방 ✗) · 미결 = 아직 가르지 못했다. 옛 뜻풀이는 원래 주장인지 정정된 이해인지
  말하지 않아 도구 결함 정정 등에서 `확증` 이 모호했다(2차 저작자 지적).
- `출처` 는 LINEAGE 문서 · evidence_candidates · artifacts 경로여야 한다(`HINT_EVENT_SOURCE_OUTSIDE_LINEAGE`). 00 §0.3 벽 지도와 00 §0.5 노브
  지위는 이 블록들에서 기계가 파생한다(파생 블록이 낡으면 `HINT_DERIVED_BLOCK_STALE` — continue 가 refresh 한다). 어휘 정본은 `template.py`
  의 `EVENT_KINDS`·`EVENT_REQUIRED`·`TRANSFER_CLASSES`·`VALUE_STATUSES` 이고 범례도 거기서 렌더한다.

### 8.3 원문 발췌 — 허용하되 글자 그대로 (D2 · X16 · D-c)

"원문 전재 금지 → 재저작" 규칙은 **폐기**했다 — 재저작한 사실 3건이 틀렸고 기계가 전사한 곳(패치 헤더 · compose 주석 · 인증서 파싱)은 오기
0 이었다(F7). 발췌 무결성이 재저작 오기를 구조적으로 막는 핵심 게이트다. 다만 "기계 전사는 오기 0" 은 **일반 전제가 아니다** — v6 라이브에서
FACT 블록 오기가 나왔으므로(§6.7) v7 은 FACT 도 사실 검증 대상에 넣었다.

- 형식: `> [원문] <문서 stem> §<절>` 다음 줄부터 `> ` 인용 줄. 해설은 발췌 **밖**에 쓴다.
- **출처 정규화**(2026-09-22 S2 round 2 · `template.normalize_source` — 발췌 도우미 출력과 린터 대조가 같은 함수): `\n` 으로만 줄을 가른다
  (유니버설 개행 ✗ — tqdm `\r` 가 줄이 되어 `excerpt --lines` 번호가 grep 과 달랐다) · 줄 끝 CRLF 의 `\r` 를 떼고 줄 안의 `\r` 진행 조각은
  **마지막 조각만** · ANSI 제어열 제거. 인용은 정규화된 원문의 부분 문자열이어야 한다(ESC 를 되살리거나 `\r` 에 덮인 조각을 인용하면 불일치).
- **기계 치환**(결정론 · 긴 리터럴 우선): 운영자 절대경로 → `<manifest.<field>>`·`<repo>`·`<home>`·`<abs-path>` · 호스트명·사설 IP → `<node:<역할>>`·
  `<priv-ip>`·`<host>` · NIC·HCA 장치 이름(manifest `*_iface` · `hca_devices[]` 등 · 숫자를 품은 장치 모양만) → 한 노드의 nodes[] 에만
  있는 이름 = `<nic:<역할>>` · 최상위 공통 설정(interconnect — 렌더러가 `.env.interconnect` 하나로 두 노드에 같이 보낸다) 또는 둘 이상의 노드가
  가진 이름 = `<nic:cluster>`(2026-09-22 적대 리뷰: 1판은 최상위를 `self_role`=main 으로 붙여 서브 Ray worker 로그 줄이 `<nic:main>` 을
  읽었다 — 도구가 만든 오귀속) · 사람 식별자 → `<redacted>`. NIC 치환은 **스캐너 체계
  변경이 아니라 치환 추가**다(P3 — NIC 이름은 스캔 대상이 아니다 · 1.5 PROMPT 가 NIC 이름을 금하는데 로그 발췌가 그대로 실었다). 치환표는 기존 env 형상화 · PII 4종 규칙을 재사용한다(새 체계 ✗ · P3). 저작자는
  `hint.py excerpt --draft <draft> --source <stem|경로|artifacts 파일명|<도구>@<rev12>> [--lines a-b] [--numbered]` 로 **린터가 대조하는 치환 후
  원문**을 받아 옮긴다(자리표시를 추측하지 않는다 · `--numbered` = 줄마다 `L<n>` — 린터가 `§L<a>-<b>` 로 대조하는 **같은** 번호 · 2026-09-22
  S2 round 3: 2차 저작자가 번호를 얻으려 원본을 따로 grep 했다).
- **무결성**: 인용 본문(공백 정규화)이 출처(치환 후 · 공백 정규화)의 부분 문자열이어야 한다 → 아니면 `HINT_EXCERPT_MISMATCH`. 출처 stem 은
  LINEAGE 문서 · artifacts 로 **유일하게** 해소돼야 한다 → 아니면 `HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE`.
- **측정 도구 스냅샷**(2026-09-22 S2 round 3): publish 가 측정 시각 이전 마지막 커밋의 측정 도구 원문(`git show <rev>:<경로>` 바이트 —
  sweep_bench · run_bench · lite_bench · 드라이버)을 draft `inputs/sources/<도구>@<rev12>` 에 쓰고 LINEAGE `evidence_candidates`
  (`kind: tool-source@rev` · `origin: git:<rev>:<경로>`)로 올린다. 발췌 출처 토큰은 `<도구>@<rev12>`(또는 `inputs/sources/…`)이고 **LINEAGE 에
  등재되고 draft 에 파일이 있을 때만** 해소된다(스냅샷 모양의 토큰은 저장소 · artifacts 로 흘려 다른 파일에 붙이지 않는다). 확장자가 `@<rev>`
  뒤에 가려지므로 절 표지는 언제나 `§L<a>-<b>` 다. 스냅샷은 배포 zip 밖이다 — 수신자는 03 §3.4 사실 블록의 `git show` 좌표로 같은 바이트를 얻는다.
  그래서 린터는 스냅샷 **파일**이 그 origin(`git show <rev>:<경로>`)과 **바이트 동일**할 때만 출처로 쓴다 — 다르면
  `HINT_EXCERPT_SNAPSHOT_DRIFT`(저작 중 스냅샷을 고치고 발췌를 거기 맞추면 발췌 대조는 통과하고 수신자의 `git show` 는 다른 바이트를 낸다 ·
  2026-09-22 round 3 적대 리뷰) · origin 모양 결함 · git 에서 못 읽음 = `HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE`(통과 ✗). `hint.py excerpt` 도
  같은 대조로 거부한다.
- **절 표지(D-c · 2026-09-22)**: 마크다운 출처면 `§<절>` 이 그 문서 제목 줄(`#`…)의 정규화 접두와 맞아야 한다(번호만 · 번호+제목) → 아니면
  `HINT_EXCERPT_SECTION_UNKNOWN`. 본문만 맞고 절 표지가 틀리면 수신자가 원문을 찾아가지 못한다.
- **비-마크다운 출처의 절 표지 = 줄 범위**(S2 round 2): 로그 · .sh · Dockerfile · requirements · JSON 발췌의 머리는 `§L<a>-<b>`(정규화 뒤 줄
  번호 = `hint.py excerpt --lines a-b`)다 — 다른 모양 `HINT_EXCERPT_SECTION_SHAPE`(1차: `§헤더` · `§엔진 로그` 를 린터가 받아 자리를 찾을 수
  없었다) · 범위 밖 · 거꾸로 `HINT_EXCERPT_LINES_OUT_OF_RANGE` · 인용이 원문에는 있으나 그 줄들 안이 아님 `HINT_EXCERPT_OUTSIDE_LINES` ·
  범위가 인용 줄 수 + 2 보다 넓음 `HINT_EXCERPT_LINES_TOO_WIDE`(넓은 범위는 자리를 가리키지 못한다).
- **서명 조각(D-c)**: wall · misdiagnosis 의 `서명` 은 출처 원문의 글자 그대로여야 하고(`HINT_SIGNATURE_MISMATCH`), `…` 로 나눈 각 조각은 공백
  제외 **12자 이상**이어야 원문 증거로 친다(`HINT_SIGNATURE_TOO_SHORT` — 한두 글자 조각은 어느 출처에나 부분 문자열로 있다). 짧은 실제 서명은
  앞뒤 원문을 더 인용해 채운다.
- **상한(X16)**: 출처 문서당 40줄 · 전체 발췌 합 400줄(`HINT_EXCERPT_SOURCE_LIMIT` · `HINT_EXCERPT_TOTAL_LIMIT`). 발췌는 PII 4종 게이트를
  그대로 통과해야 한다(P3 는 체계를 바꾸지 않는다는 뜻이지 게이트 해제가 아니다).

### 8.4 값의 지위 커버리지 (F8 · D-b)

01 §1.3 표의 **서빙 노브마다** `kind: value-status` 블록이 정확히 1개 있고 `값` 이 표의 값과 글자 그대로 같아야 한다(`HINT_VALUE_STATUS_GAP` ·
`_DUPLICATE` · `_VALUE_MISMATCH`). 옛 L3(전이등급 태깅) · B1(재측정 어휘)은 템플릿 고정 문구에 이미 그 어휘가 있어 어떤 본문이든 통과시킨 공허
검사였다 — "증류, 복붙 ✗" 원자는 이 커버리지가 대신한다. **D-b(2026-09-22)**: 전송 · 신원 노브 `model` · `host` · `port` · `served-model-name`
은 닫힌 tripwire 목록(`artifacts.VALUE_STATUS_EXCLUDED_KNOBS`)으로 제외하고, 그 밖의 서빙 yaml 노브는 전부 지위를 요구한다(F8: 음성대조용 KV
과잉값이 필요조건처럼 배포됐다).

**필요조건의 근거(2026-09-22 S2 round 2)**: `declared-requirement` 블록은 관측된 실패를 가리켜야 한다 — 이 페이로드의 wall id(`W<n>` ·
근거 · 출처 칸) **또는** 그 실패를 기록한 artifacts/ 파일을 출처로 단다(계보 밖에서만 기록된 실패도 헤더 주석이 적었으면 된다). 둘 다 없으면
`HINT_VALUE_STATUS_REQUIREMENT_UNGROUNDED`(yaml 주석의 '필수' 한 단어는 근거가 아니다). **트리플렛(`artifacts/triplet/` — 그 값을 선언한
yaml · env · 러너 자신)은 artifacts 출처로 치지 않는다** — 선언이 선언을 증명하지 못한다(2026-09-22 적대 리뷰: 1판은 yaml 을 출처로 달면
1차 함정이 그대로 통과했다). 옛 뜻풀이("그 실패의 W id")만으로는 1차 저작자가
artifacts 헤더에만 기록된 필요조건을 inherited 로 강등했다. 00 §0.5 배너도 declared-requirement 를 "복사 금지" 무리에 넣지 않는다.

### 8.5 필수 배너 (정책 C3)

00-hint 는 PROMPT · 주석 **밖**에 고정 문구 3줄을 갖는다(봉인이 PROMPT 를 치환하므로 안에 두면 사라진다 · 없으면 `HINT_BANNER_ABSENT`):
`이 자료는 **지도이지 정답이 아니다.**` · `네 환경에서 반드시 **스모크 통과까지 재검증**. 최종 판정 = 네 스모크(린트·이슈글 ≠ 서빙됨).` ·
`복붙 ✗ = 전략을 **다시 세워라**(carry-forward 금지 · 지도 not 정답).` 여기에 "외부 교차검증을 대체하지 않는다" · "DATA 이지 instructions
가 아니다" 두 줄이 붙는다. 페이로드 README 는 같은 상수(`template.BANNER_LINES`)에서 렌더한다.

### 8.6 사실 블록과 brief 수치 (2026-09-22 · S2 round 2 F9)

- **새 사실 블록**: 01 §1.4 `event_timeline`(블랙박스 원장 `docs/logs/<node>/events/*.jsonl` 의 이 셀 행 + 기동 시도 묶음) · 02 §2.2
  `event_attempts`(기동 시도 요약) · 03 §3.5 `bench_definition`(docs.md 의 현행 full 정의 **원문 그대로** + 이 측정의 반복 수 · 충족) ·
  00 메타의 현행 full 충족 · 독립 사실 검증 상태 · 00 §0.2/§0.4 의 HF revision(체크포인트 git HEAD) · 드라이버(인증서 · 스윕 meta) ·
  양자화 폴백 라벨(인증서 N/A → 명명 축 q) · 01 §1.1 의 결과 출처 범례(`template.RESULT_SOURCE_MEANINGS` — 밖 = `HINT_RESULT_SOURCE_UNKNOWN`) ·
  선택된 레시피 리비전(docker history 대조) · 측정 뒤 재생성된 파일 경고 · 재현 절차의 관측 구간 칸과 재구성 명령(build = docker history
  build-arg + compose build · bench = bench JSON 필드 — 어느 쪽도 실행 원문이 아니다). 1차 저작자가 "미관측 · 미기록" 으로 적은 빈칸
  (부류 7)을 기계가 채운다.
- **사실 블록의 절 자리표시**(`03-benchmark.md §3.x` 류)는 생산자 결함이다 — `HINT_FACT_PLACEHOLDER` 로 막는다(사실 블록은 손으로 고칠 수
  없으므로 facts 조립을 고친다).
- **brief 수치**: 00 §0.1(= annotation brief)의 소수 · 세 자리 이상 수치는 이 페이로드의 **기계** 사실 블록(03 표 포함)에 **글자 그대로** 있어야
  한다(`HINT_BRIEF_NUMBER_UNGROUNDED` — 단위 환산 `20480MiB ← 21474836480` · 반올림 · 다른 셀의 수치는 새 수치다). 파생 블록(00 §0.3 · §0.5 —
  저작자 hint-event 를 옮긴 표)은 허용 집합이 아니다(저작 수치가 스스로를 근거 짓지 않는다) · 소수는 글자 단위가 붙어도(`38.8t/s`) 대조한다.
  식별자 조각(`v0.29.0rc6`) · 약칭(`262k`) · 한두 자리 서수는 대조하지 않는다. 수치 대신 V/W id 로 가리켜도 된다.
- **기동 시도 묶음**(01 §1.4 · 02 §2.2): `budget_declare` 행마다 한 시도 · 원장 kind 는 **정확 일치**로 가른다(`budget_declare_rejected` 는
  새 시도가 아니다 · `budget_clear` · `budget_expired` = 닫힘) · 사살 · 트립 · 거부(`watchdog_*` · `thermal_trip` · `*_kill_ack` · `earlyoom_kill` ·
  `budget_*rejected` · `budget_blocked`)는 닫힘과 따로 센다(뒤따르는 clear 에 덮이지 않는다 — 1차 오진 부류 "hang 대 워치독 사살").
- **3.5 비교 규칙 = 인증서 자신의 규칙**: 강한 일치 키(인증서 6키) 불일치 = 무효 · 인증서 `# --- 소프트 지문` 블록의 키 전부(driver · cuda ·
  image · max-len · kv-bytes · kv-dtype · gmu · moe · attention · ple · ngc · bench_tool …) 불일치 = stale("비교 가능(형상 비교 · stale)" +
  다른 키 전부 나열) · 부하 조건(입출력 길이 · 동시성 · 요청 수 · spec 상태 · 도구) 불일치 = 비교 불가. 옛 PROMPT 의 "digest · NGC · torch · CUDA"
  목록은 인증서 스키마와 맞지 않아 PLE · KV · gmu · max-len 만 다른 자매 셀이 어느 무리에도 들지 않았다.
  **spec 상태 = 켜짐/꺼짐과 방식**(비교 조건)이고 **잰 acceptance length 는 측정 결과(출력)** 다(2026-09-22 S2 round 3): 둘 다 spec 이 켜져
  있으면 acceptance length 가 달라도 비교 불가가 아니다 — 두 측정의 acceptance length 비를 decode 처리량 비 옆에 **비로** 나란히 적고, 처리량
  차이가 그 비로 설명되는지(설명된다 · 안 된다 · 판정 불가)를 적는다(round 2 PROMPT 는 acceptance length 를 조건 목록에 넣어 모든 spec-on 비교를
  비교 불가로 만들었다 — 2차 저작자가 계약과 PROMPT 중 하나를 골라야 했다). 같은 셀의 이전 측정은 **레벨 전부**를 이 측정의 같은 레벨 옆에
  나란히 싣는다(한 레벨만 떼어 비교 ✗ — 2차 여정 채점 #19: 레거시 곡선의 나머지 레벨이 빠졌다).
- **새 사실 블록(2026-09-22 S2 round 3 · 공유 사실 계약)**: 01 §1.4 `measurement_env`(측정한 실행의 엔진 로그 env echo — 측정 당시 값의 정본 ·
  측정 뒤 재생성된 env 형상은 싣지 않는다) · 01 §1.5 `measurement_env_nodes`(키 × 노드 접기 — 양 노드 같음은 이름만 · 다름 · 한쪽만은 행) ·
  03 §3.4 `tool_snapshots`(측정 도구 원문 스냅샷 · 리비전 · 다음 변경 · `git show` 좌표) · 00 §0.4 vLLM 버전 문자열마다 **생산자**(엔진 기동
  배너 · 인증서 강한 키 — 측정 도구가 IMAGE_TAG 에서 자른 값이면 그렇다고 · wheel 메타) · attestation 범위 줄(01 §1.1 탐침 · 01 §1.5 ABI · 00
  결손 — evidence 의 scope 그대로) · 01 §1.1 복수 리비전(`selected_revisions` — build_recipe 와 compose 추적 파일 · 근거 · 측정 뒤 커밋 수 ·
  경고) · 싣지 않은 파일(`post-measurement-regenerated` 등 사유 · 패치 아님은 개수만 · skip 은 적용 집합 표에만) · 패치별 판정 근거(스크립트
  바이트 정체 · 이미지 사본 sha256 · 적용 표지 · 정적 규칙 · 빌드 루프) · 01 §1.4 빌드 층 CreatedAt 창(소요가 아니라 층 창) · 측정 뒤 재생성 형상이
  있으면 `render` 단계 경고(지금의 렌더러가 측정 당시와 다른 키 · 값을 낼 수 있다 — 측정 env 관측과 대조). 0.2 라벨은
  "vLLM(측정 강식별 키 `vllm_version`)" 이다 — 옛 "엔진 자기보고 vLLM" 은 사후 사실 검증 오답이었다.
- **PROMPT 개정(round 3)**: 0.2 · 2.5 = 이 형상이 버티는 기전 대 다른 형상이 실패한 이유 + 지위(확정 · 가설(강등) · 미결) · 형상을 연 설정
  변경은 전부(2차 여정 #40 부재 · 사후 사실 검증 M6) · 2.4 = 계보에 교정 묶음(항목 id 목록)이 있으면 이 셀에 영향을 준 항목을 id 로(#24) ·
  현재지위 = 원래 주장의 지위 · 3.4 = bench JSON 에 없는 도구 인자는 측정 도구 스냅샷에서 인용(레그별) · 1.2 (d) = 트립와이어가 맞지 않을 때
  skip 인지 빌드 중단인지를 스크립트 원문으로(사후 사실 검증: 30 의 skip 주장이 틀렸다).
- **봉인 = 수신자 zip**: 봉인 뒤 문서마다 조건이 성립하는 템플릿 PROMPT 수 = 질문 줄 수(각 1줄) · 지시 필드 줄 0 · SELFCHECK 0 · `<<AGENT:` 0
  (자체검사 ★ 술어 — 봉인 전 draft 에 같은 술어를 대면 거부한다. 2차 바이트 채점은 봉인 전 draft 를 쟀다).

## 9. 옛 태그와의 공존 — 읽기 전용 · 리콜 금지 (P1 · P2)

원격에 올라간 hint 태그는 **교정 · 삭제 · 재발행하지 않는다**(P1 사용자 전제: "이미 원격 저장소로 올라간 hint태그는 절대로 교정작업을 하지
않을 것"). 개정판은 새 이름의 신규 발행으로만 나온다. 여러 로컬 저장소가 원격에 태그를 올린다는 사실은 **인지만** 한다(P2).

| 문법 세대(`naming.grammar_of`) | 모양 | 시기 |
|---|---|---|
| `v7` | `<hw>-<G>g<N>n-<role>-<target>[-bare]` + 결정론 `q·len·kv` + 근거 붙은 꼬리 + 중복 시 `-t<YYMMDDHHMM>` · 페이로드 커밋의 부모 = 안내 커밋 · `hint-payload/v7` | 2026-09-29 ~ |
| `v6` | `<hw>-<G>g<N>n-<role>-<target>` + 고정 6축 레시피(v7 파서에서 3축 + 꼬리 3토큰으로 읽힌다) · 페이로드 커밋 체인 · `hint-payload/v6` | 2026-09-22 ~ 09-28 |
| `legacy-5seg-node` | arch `<hw>-<main\|sub\|cluster>-<target>`(예 `gb10x2-cluster-native`) | 2026-09-06 ~ 09-21 |
| `legacy-5seg` | arch `<hw>-<target>` · 노드 축 없음(예 `gb10-sim-h100`) | 2026-09-04 ~ 09-06 |
| `legacy-4seg` | `hint/<vllm>/<model>/<arch>` · 레시피 세그먼트 없음 | 2026-09-04 이전 |
| `unknown` | 5세그먼트지만 어느 세대에도 맞지 않음(타 발행처) | — |

- 파서는 옛 세대를 **읽기 전용**으로 수용한다 — 카탈로그 나열과 `문법` 열이 목적이며 판정하지 않는다(D10). 옛 태그의 결함(`REF_ABSENT` 류 ·
  로컬 오브젝트 부재 · lightweight)은 신규 발행을 막지 않는다(AC6).
- 옛 형식 페이로드는 annotated 본문이 지도였고 zip 은 3항목 문서였다. v6 는 지도를 zip 의 `00-hint.md` 로 옮겼다 — 수신자는 태그의 `문법` 과
  페이로드 `format` 으로 읽는 법을 가른다. v6 태그의 footer 는 v1(`certificate_ref`)이고 LINEAGE 는 전체이며 부모 체인형이다 — v7 발행기는 그것을
  읽기만 하고 판정 · 교정하지 않는다(v6 4태그 · 옛 DS4F 계열 포함 · P1).
- **레거시 v1 은퇴의 논거**(2026-09-01 · 62건 회수는 사람의 결정 · 원본은 `seed/hint_tags_backup_26090108/`)는 리콜 금지의 선례로 남긴다:
  만들어질 당시 증거를 요구하지 않았으므로 부재가 곧 허위는 아니다 · 이미 배포돼 소급 차단의 실익이 0 · 태그 오브젝트 재작성은 원격 이력을
  흔드는 되돌리기 어려운 작업이다.

## 10. 알려진 한계 (정직 기재)

- **D-d native 평면**(2026-09-23 해소 · v7 구조 통일): native 정문(`plan_26092311` N1)의 serve proof producer 가 평면을 명시하고, v7 부터 native 셀은
  Docker 와 같은 슬롯을 싣는다(§3.3). 남은 규칙은 하나 — 평면 신호가 **없으면** `HINT_PLANE_UNDERIVABLE` 로 fail-closed 다("신호 없음 = native" 는
  결정 경로의 침묵 폴백이라 쓰지 않는다). native 의 compose · 러너는 실행 검증되지 않은 생성물(`generated-unverified`)이며, 수신자가 그것을 검증된
  것으로 오인할 위험은 세 자리 표시 + 00 머리 요약으로만 줄인다(plan_26092908 R3).
- **꼬리의 자유도**(plan_26092908 R1): 같은 셀을 PC · Agent 마다 다른 꼬리로 부를 수 있다(검색 분산). 결정론부 매칭이 1차이고 꼬리는 근거 대조로
  거짓만 막는다 — 표기 흔들림은 수용한다(U5 의 대가).
- **브랜치 형식 전환**은 사람 승인 명령(`branch-transition`)이다 — 원격 안내가 v7 이 아닌 동안 모든 발행이 `HINT_BRANCH_FORMAT_MISMATCH` 로 선다.
- **D-e 서브 단독 셀**: 서브에서만 잰 셀은 메인이 그 관측 원시(serve_proof · post_health · lite raw)를 볼 수 없어 `HINT_QUALIFICATION_UNOBSERVED`
  로 막힌다. 서브 관측을 문서기반 회수로 받는 설계가 열린 항목이다(예약 결손 코드 `HINT_MISSING_SUB_TRIPLET` 의 발행자도 그 설계가 정한다).
  서브의 `hint_inputs` 사이드카는 발행기가 읽지 않는다. E2E 는 cluster 셀을 쓴다.
- **4마디 릴리스 × PII**: D-g 가 4마디 릴리스 이름을 허용했지만, 두 번째 마디가 `10` 인 4마디 릴리스는 사설 IPv4 패턴의 3옥텟 접두 매치에
  걸린다 — 그 이름은 커밋 · 봉인에서 반드시 거부되므로 `publish` 가 저작 전에 `HINT_TAG_NAME_PII` 로 먼저 막는다. 스캐너 체계 변경(버전
  토큰 면제 등)은 사람 결정이다(P3).
- **draft 의존 검증**: 단독 `verify` · `push` 는 그 태그를 봉인한 draft 가 있어야 서사 린트까지 돈다 — draft 를 잃으면 재발행(새 이름)이다.
- **push 뒤 카탈로그 실패**: 카탈로그 파생이 push 뒤에 실패하면 publish 위상이 기록되지 않은 채 남는다 — continue 재실행이 원격 확인
  (멱등) → 카탈로그 → 위상을 잇는다(원인을 고친 뒤).
- **publish 의 PII 입력 재사용**: 캠페인 셀의 `continue` 는 promotion_target 을 적으며 finalize 를 다시 돌릴 때 publish 때 도구가 쓴
  `inputs/pii.json` 을 그대로 쓴다 — publish 뒤 편집된 서사 증거 문서(plan · devlog · testlog)는 다시 스캔되지 않는다. 배포되는
  바이트(페이로드 트리 · 커밋 메시지 · annotation)는 커밋·봉인·검증이 매번 다시 스캔한다.
- **발행 호스트의 PyYAML**: manifest · 셀 config · 서빙 yaml 은 소유 로더(terraforming `manifest_contract`)로 읽으므로 발행(메인)에는
  PyYAML 이 필요하다. hintlib 자체와 수신자 `match` 는 stdlib 만 쓴다.
- **PROVENANCE.source_anchor** 는 publish 때의 HEAD 이며 워킹트리가 dirty 여도 표시하지 않는다(스키마에 dirty 칸 없음) — 01 의 recipe-vs-image
  경고만 그 사실을 말한다.
- `PAYLOAD.naming` 에 naming facts 원문은 싣지 않는다 — 로컬 검증의 이름 재대조는 축 · 꼬리 · timestamp 에서 이름을 다시 조립하는 검사까지만 한다.
- **DS4F 재현 키트의 tool calling**(V16): 그 셀의 서빙 yaml 에는 `enable-auto-tool-choice` 가 없어 재현 키트로는 tool calling 이 되지 않는다 —
  태그에는 기재만 한다(사용자 결정 "기재만 유지").

## 11. 개정 이력

| 판 | 날짜 | 요지 |
|---|---|---|
| v1 | ~2026-07 | 암묵 규약. 게이트가 형식은 막고 "서빙되었다는 주장이 사실인가" 는 검사하지 않았다 — 2026-07-31 신규 태그 1개가 레거시 23개의 형식 미비로 영구 차단되며 드러났다(이 계약의 명문화 이유 · D10 의 원 선례) |
| v2 | 2026-07-31 | 명문 · 차단 사유를 형식 → 증거 부재·위조로(§2) · 소급 금지 |
| v2.1 | 2026-08-01 | 성능 REFUTE 의 사람 중단권 · perf_waiver(§3.2) |
| v3 | 2026-09-01 | 분류기 4등급 → 2등급 · tagger 기본값 = 합성 신원 · PII 대상이 페이로드 트리 전량 · 태그 → hint 브랜치 페이로드 커밋 · 레거시 v1 62건 회수(§9) |
| v4 | 2026-09-06 | 필수는 여정 하나 · 부재는 기재 · 한 태그 = 한 노드 형상(arch 노드 축) · 벤치 절 결정론 파싱 |
| v5 | 2026-09-07 | 층별 정합(A 항상 필수 · B `hint_map_only` · C manifest) · 결손 가시성 3곳 · 태그 앵커 검사 집행 · 노드축↔인증서 대조 · ACTIVE 비참조 · 2026-09-14 lite 셀 통로 |
| v6.1 | 2026-09-22 | plan_26092119 S2 round 2(오프라인 재생 채점의 처방): 발행 전 **독립 사실 검증 게이트**(§6.7 · 7부류 · 사람 면제만 탈출) · 템플릿 저작 자기점검(SELFCHECK · 봉인이 지운다) · `hint.py refresh` · 발췌 출처 정규화(`\n` · `\r` · ANSI) · 비-마크다운 `§L<a>-<b>` · NIC 장치 이름 치환 · brief 수치 대조 · declared-requirement 근거 · 3.5 비교 = 인증서 소프트 지문 · 새 사실 블록(§8.6) · 같은 날 적대 리뷰 정정(최상위 NIC = `<nic:cluster>` · declared-requirement 근거에서 트리플렛 제외 · brief 허용 집합에서 파생 블록 제외 · 기동 시도 kind 정확 일치 · 면제 문구 = 사유대로) · 통합 정정(빈 이벤트 타임라인 = 원장 부재와 행 0 을 구분하지 않는다고 적는다 · `--docker-probe` 원장 = create+cp · §6.7 면제 문구 = 사유코드대로) — 형식 문자열(`hint-payload/v6`)과 이름 문법은 그대로다 |
| v6.2 | 2026-09-22 | plan_26092119 S2 round 3(2차 채점의 처방): **이미지 탐침 = publish 기본**(시작하지 않는 컨테이너 create · cp · rm · `--docker-read-only` 가 끈다 · `--docker-probe` = no-op 옛 이름 · 정책 자체검사 docker 계약 동행 · §6) · `excerpt --numbered` · 측정 도구 스냅샷 발췌 출처(§8.3) · 새 사실 블록 · 3.5 acceptance length = 출력 · PROMPT 개정 · 현재지위 = 원래 주장의 지위 · PAYLOAD 새 키(§7.2 · §8.6) · 같은 날 적대 리뷰 정정(스냅샷 = origin 바이트 동일할 때만 출처 ·
  01 §1.5 노드 접기는 Ray 가 접은 줄에서 다른 쪽 부재를 단언하지 않는다 · 탐침 규칙 단일 소유 = artifacts 실행기) — 형식 문자열(`hint-payload/v6`)과 이름 문법은 그대로다 |
| **v7** | **2026-09-29** | plan_26092908(v6 4태그 적대 검증 · 블라인드 수신자 3 · 세션 채굴 2 · 구조 감사 1 → 사용자 결정 U1~U9): 이름 = 결정론 `q·len·kv` + Agent 꼬리(`tail.json` · 형식 · 뜻 · 근거 대조 · v6 뒤 3축 모양 거부) + 원격·로컬 중복 시 `-t<YYMMDDHHMM>`(§4) · native 구조 통일과 파일 단위 verification · generated_by · relevance(세 자리 일치 · §3.3) · 판정 기계 표면(`measurement.verdict` · 판정 행 · 배너 · 카탈로그 판정 열 · §3.0.2) · footer v2(`bench_ref` · `bench_kind` · §7.3) · 인증서 요구 = explicit ∧ PASS 만(§3.0.1 · completion_gate) · 관측 > 선언(driver_conflict · §7.2) · 사실 검증이 FACT 도(`target` · `refacts` · §6.7) · 페이로드 커밋 부모 = 안내 커밋 · 형식 마커 · `branch-transition`(§6.5) · 봉인 뒤 verify 출처 변경 = INFO(§6.3) · LINEAGE 요약/전체 분리(§8.1) · 거부된 publish 의 시각 채택 · `--node` 자동 · 카탈로그 자동 커밋(§6) · 새 결손 코드 3종 · V15 잔재 정리(§10 D-d · 평면 코드명 통일 `HINT_PLANE_UNDERIVABLE`) — 형식 `hint-payload/v7` · PAYLOAD schema 3 |
| **v6** | **2026-09-22** | plan_26092119: 셀 1개 = 태그 1개 · 단일 진입 `hint.py`(옛 CLI 5종 · families · 스캐폴드 템플릿 제거 · shim ✗) · 이름 전량 도구 파생(5세그먼트 · 어휘표 · 전 축 필수) · 발행 자격 = 관측 · 승인 = 셀별 사전 기록(O6) · A 층 = 평면별 재현 입력 "쓰인 것만" · 지도가 zip 안(00-hint) · 계보 서사 PROMPT 템플릿 · 발췌 허용 + 무결성 · 값의 지위 · 커밋 신원도 합성 · push = 태그 1개(브랜치 ✗) · 게이트 = 이번 발행분만 |

v5 의 옛 절 번호 인용(`계약 v5 §3` C행 · `§3.-1` 앵커 검사 · `§3.-2` 노드축 대조)은 각각 v6 의 §3(C행 = §3 표의 C 층 · 통로는 §3.0.1) ·
§6.5 · §4 의 "노드 축 ↔ 측정 대조" 항목으로 옮겨졌다. `§6` 의 "태그는 hint 브랜치의 페이로드 커밋을 가리킨다" 는 §6.5 에 있고 v7 에서
"안내 커밋을 부모로 한 페이로드 커밋" 으로 개정됐다(조상 검사 `HINT_ANCHOR_NOT_ON_HINT_BRANCH` → 부모 검사 `HINT_ANCHOR_PARENT_NOT_GUIDE`).
