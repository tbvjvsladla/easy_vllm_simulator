# docs.md — 문서 작성 규약 (plan · devlog · testlog · simlog · benchmark · report)

> 이 파일은 워크스페이스의 **문서 6종 역할·명명·구조 규칙**이다. "항상 참인 사실"은 루트 `CLAUDE.md`,
> 다단계 전파 절차는 `.claude/rules/workflow.md`. 이 규약은 기존 `docs/` 관행을 명문화한 것이다(헌법).
> 모든 작업 산출 문서는 아래 규칙을 따른다.

## 1. 공통 명명 규칙 (결정론적)

```
경로:  docs/<type>/<type>_<YYYYMMDDHH>_<seq>_<주제>.md
```

- **type** ∈ `plan` · `devlog` · `testlog`. **type별 서브디렉토리 + 파일명 접두사**를 모두 둔다
  (예: `docs/devlog/devlog_...`). 평면(`docs/파일.md`) 배치 금지.
- **YYYYMMDDHH**: 작성 일시 절대표기(예 `2026060814` = 2026-06-08 14시). 시각(HH)까지 기록. 상대날짜(오늘/어제) 금지.
- **seq**: 같은 일시·같은 type 내 일련번호, `1`부터 증가. (같은 시각 2번째 devlog → `_2_`.)
- **주제**: 한국어, 밑줄(`_`) 구분, 내용 식별 가능하게 간결히. 버전·대상 포함 권장
  (예: `Phase2_소스빌드_vLLM0.22.1_검증`).
- 예시:
  - `docs/plan/plan_2026060721_7_Phase2_소스빌드_vLLM0.22.1_실행계획.md`
  - `docs/devlog/devlog_2026060812_2_멀티노드_소스빌드_확장_검증.md`
  - `docs/testlog/testlog_2026060812_2_멀티노드_소스빌드_vLLM0.22.1_검증.md`

- **simlog만 예외 — 파일이 아니라 폴더 1개 = 1 run**(다중 산출물 vault). `<type>_` 파일 접두사 없이
  **run 디렉토리**를 만든다. seq·주제 규칙은 동일.
  ```
  경로:  docs/simlog/<YYYYMMDDHH>_<seq>_<주제>/   ← run 디렉토리 (파일 아님)
  ```
  - 예: `docs/simlog/2026062121_1_vLLM0.22.1_KV클램프_시뮬/`

- **benchmark 도 예외 — 산출물 종류가 파일 접두사**(`report_`/`benchmark_`). full-런 자동 발행물이라
  `<type>_<YYYYMMDDHH>_` 대신 **대상 조합**을 파일명에 담는다(재발행 시 덮어쓰기 = 최신 계측 1개 유지).
  run 디렉토리 아님(평면 파일). 정본 = 스킬 `adversarial-benchmark` full 모드.
  ```
  docs/benchmark/report_<model>_<gpu>_<vllm>.md      ← 사람용 (항상 · PASS/FAIL 무관)
  docs/benchmark/benchmark_<model>_<gpu>_<vllm>.yaml ← 인증서 (PASS시만)
  ```
  - 예: `docs/benchmark/report_deepseek-v4-flash_GB10_0.24.0.md`

- **report 도 예외 — 주제 슬러그만**(날짜·seq 없음). 사람이 자유 발행하는 아웃바운드 공지라
  README 처럼 **갱신·덮어쓰기로 최신본 1개**를 유지한다(시점 기록이 목적 아님 — 산출물 성격이 명명을 정한다).
  ```
  docs/report/<주제-슬러그>.<html|md>   ← kebab-case (예: rtxpro6000-benchmark-explorer.html)
  ```

## 2. 6종 역할 (무엇을 · 언제 · 어디에)

### plan/ — 계획서 (작업 **착수 전**)
- **역할**: 단계/Phase 작업의 계획·설계·접근방식. **사람 검토(HITL) 대상** — 실행 전 합의용.
- **시점**: 실행 이전. (부트스트랩: `docs/plan` 작성 → 사람 검토 → 실행 → 기록 → 다음.)
- **담는 것**: 목표·범위, 결정론적 해소값 설계, 단계별 절차, 리스크, 검증(합격) 기준.
- **두 무게, 한 장르 (plan_2026063018_1 · 로그=에이전트 철학)**: ① *스킬·헌법 개발*(우리) = **무거운** plan(전 섹션). ② **배포자 온보딩**(`terraforming_node` init-mode)도 **같은 `docs/plan/` 장르**로 발행한다(경량 별도장르 ✗ — wiki-desk 색인·습관화 = 자기개선 루프 수동 기둥 첫 접점). 다만 비전공 배포자 부담을 줄이기 위해 **에이전트가 인터뷰 답에서 대신 초안**(토폴로지·획득모드·스캔할일·branch정합·완료 시 Flag)하고 **배포자가 자기-HITL 승인** — *무게가 아니라 경험*을 매끄럽게(약간의 강요는 의도된 철학). 온보딩 plan 의 실제 깊이는 작업 복잡도에 비례(단일노드 스캔이면 짧음).

### devlog/ — 작업 로그 (작업 **중·후**)
- **역할**: 실제 수행한 **작업 내역·결정·전파의 서사**("무엇을 했나").
- **담는 것**: 작업 흐름, 핵심 사건/결정(+근거), 교훈, **최종 상태**(미커밋·다음 작업 명시).
- **원시맥락 범주(다음 세션 warm-start 용 — plan_2026070208_1 Phase 2)**: 서사 요약만으로는 다음 세션이
  복구 못 하는 것들을 명시 수록한다 — **시도-폐기 경로**(무엇을 시도했고 왜 버렸나 · 음성결과 포함),
  핵심 **재현 커맨드 verbatim**, verbatim 에러 시그니처(가변 로그는 simlog 인용).
- **미완결 세션 devlog 필수 섹션 = "최종 상태 + 재개 지침"**(devlog_2026070207_1 §8 관행의 codify):
  ① 디스크/노드 실상태(이미지 태그·미커밋 분류·관련 파일 목록) ② 의사결정 대기 항목(후보별 정확 SHA/값)
  ③ **재개 커맨드 verbatim**(+진단 grep 패턴) ④ 예상 벽. 완결 세션은 ①만으로 충분.

### testlog/ — 검증 로그 (검증 **결과·증거**)
- **역할**: 빌드/스모크/실험의 **증거와 판정**("동작을 확인했나").
- **담는 것**: 목적, 전제(resolve값), 실행 커맨드, 관측(로그·수치), **합격/실패 판정**, 실패 시 근본원인,
  **환경 스냅샷**(이미지 태그·핵심 env 실값·판정 시점 config — trial 스윕이면 per-trial config 는 simlog run
  에 사본 적재하고 여기엔 run 경로 인용; "S2가 정확히 어떤 yaml/플래그였나"를 미래 세션이 복원 가능해야 함).

### simlog/ — serve/시뮬레이션 원시증거 vault (모든 trial-loop run 산출)
- **역할**: trial-loop **한 run의 원시 증거 적재함**("실측이 정확히 무엇이었나").
  testlog가 사람용 종합 보고서라면, simlog는 그 보고서가 인용하는 **기계 생성 raw 증거**다.
  **적용 범위 = `recipe.py simulate` 산출 + 수동 serve 스윕/bump 난항의 trial 반복**(plan_2026070208_1 Phase 2
  확장 — 2026070207_1 run 의 serve_logs·key_files_snapshot 관행 승격): 반복 serve 실험이면 어느 평면이든
  per-trial 원시증거를 run 디렉토리로 남긴다. **per-trial config 사본(그 시점 yaml/플래그) 필수** — 과거
  trial 설정이 in-place 변이로 유실되지 않게(Band3 gitignored 대비).
- **구조**: 파일 1개가 아니라 **run 디렉토리 1개**(`simlog_writer.py`가 기록 — 수동 스윕은 에이전트가 동형
  구조로 적재: serve 로그 + 그 시점 config/키파일 스냅샷 + 요약). simulate run 폴더 내용(trial NN은 `01`부터):
  - `trialNN_vllm.log` — 컨테이너 docker logs 원문(`VLLM_LOGGING_CONFIG_PATH`로 `/app/simlog`에 캡처).
  - `trialNN_profile.json` — `parse_vllm_log` 실측(weights/kv/overhead GiB, 백엔드 등).
  - `trialNN_candidate.yaml` — 그 trial의 lock-set + 설정한 `kv_cache_memory_bytes`.
  - `trialNN_smoke.json` — `functional_smoke` 결과(completion/tool_call/reasoning).
  - `correction_history.jsonl` — trial 간 결정론 조정·soft 변수 폴백 1줄/건.
  - `run_summary.json` — 수렴 여부·trial_count·최종 채택 candidate·실측 분해.
- **vLLM 로깅**: 컨테이너에 `vllm_logging_config.json`(simlog_writer 템플릿)을 `VLLM_LOGGING_CONFIG_PATH`로
  주입해 로그를 simlog 경로 파일핸들러로 떨군다.

### benchmark/ — full-런 성능 계측 vault (사람용 report + 기계용 인증서)
- **역할**: adversarial-benchmark **full 모드** 종결 시 자동 발행되는 계측 vault(simlog 자매 — raw 계층 위 report 계층). 두 산출물 공존:
  - ① **사람용 report**(`report_*.md`, **항상**·PASS/FAIL 무관) = 부하 스윕 곡선(client-load·reload 0)·루프라인
    컨텍스트·환경 스냅샷. **inform-only**(verdict 를 *표시만* — 판정 권한 ✗) · 결정론 `render_report.py`(LLM 표저작 ✗) · N/A fail-soft.
  - ② **기계용 인증서**(`benchmark_*.yaml`, **PASS시만**) = "이 모델을 이 HW/config 서 테스트·통과했다"는 **flat
    계약**(중첩 ✗ — 소비자 stdlib 독해) · **carry-forward 재검증 헤더**(강한키=model/gpu/vllm/quant/topology/tp 정확일치 +
    소프트지문=driver/cuda/image/max-len/kv-bytes/gmu/moe 불일치 시 stale) 필수 · 결정론 `publish_benchmark_record.py`.
  - ③ **Max envelope 보고서**(`max_envelope_*.md`, **Max 오퍼레이션 실행시**) = HW 안전-최대 컨텍스트(max-model-len)
    특성화 · **inform-only**. Max = 벤치마커 인프라 공유 **별도 오퍼레이션**(native 모드 ✗ · 이중 게이트) · 결정론 `render_max_report.py`(SKILL.md §8.5).
- **testlog 와 경계**: benchmark=**inform-only 계측 렌더**(판정 ✗·verdict_rule 독점) ↔ testlog=**사람용 판정 서사**.
- **비용 규율**: 스윕/리치리포트는 재탐색 루프 매회차 ✗ · **종결 1회**(루프 내부는 값싼 단일점). lite 모드는
  발행 ✗(채팅 표만). 근거 = `seed/letter_2026071516_1` · `plan_2026071510_1`.

### report/ — 배포자 대상 공지 채널 (앞 5종과 직교 — 아웃바운드)
- **역할**: 메인테이너가 **배포자(클론 사용자)에게 알리고 싶은 내용**을 자유롭게 싣는 채널. 앞 5종이
  *에이전트가 발행하는 내부 작업이력*이라면, report 는 ***사람이 외부로 내보내는 공지***다.
- **트리거 없음**: 루틴·워크플로·스킬에 등록하지 않는다(자동 발행 ✗ — 사람이 내킬 때). **wiki-desk 색인 대상
  ✗**(작업이력 그래프의 노드가 아님) · **서브 미전파**(메인 전용 평면 — `references.md` 와 동형).
- **형식**: **단일파일 self-contained 반응형 HTML 권장**(강제 ✗ — 다양한 html 자유 발행). 외부 CDN·폰트·
  fetch 0 · `viewport`+`@media` · 넓은 표는 `overflow-x:auto`. 참고 예제 = `seed/rtxpro6000-benchmark-explorer.html`
  (비추적 seed — 배포본엔 부재 가능).
- **⚠ 유일한 추적 예외**(§4) + **PII 금지**(추적·배포물이므로 — 게이트 = `smoke_clone.sh` A4, 확장자 무관 전수 grep).
- 상세 규정 = `docs/report/example.md` · 근거 = `docs/plan/plan_2026071617_1`.

## 3. 작성 원칙

- **분리**: 한 작업의 *서사*는 devlog, 그 *검증 증거/판정*은 testlog로 분리. 계획은 plan.
  (검증을 동반한 작업이면 devlog + testlog 한 쌍이 보통.)
- **적용 범위**: plan은 단계/Phase 등 큰 작업에 작성(소규모 작업은 생략 가능). devlog는 의미 있는
  작업마다 남긴다. testlog는 빌드/스모크/검증을 수행했을 때 남긴다.
- **상호 참조**: 본문에 관련 문서 경로를 명기(devlog → 해당 testlog, plan → Seed/근거 등).
- **참조 체인**: `simlog`/bench JSON(원시 증거) → `benchmark report`(inform-only 계측 렌더) →
  `testlog`(인용·종합 보고서·판정) → `devlog`(서사). simlog·benchmark 는 사람이 직접 읽기보다 testlog가
  경로로 인용하는 증거·계측 저장소다. (simulate run이면 testlog 본문에 simlog run 경로 명기.)
- **사실 우선**: 절대 날짜·결정론적 값(torch 핀·NGC 태그·스모크 결과)을 명시. 추측은 "확인 필요"로 표기.
  가변 파일(헌법·스킬·코드)의 라인번호 인용 시 **literal 인용구(또는 커밋 SHA) 병기**(라인번호 단독 금지 — rot).
- **소급 배너(판정 반전 시 의무 — 앵커링 방지 · plan_2026070208_1 Phase 2)**: 후속 문서가 선행 문서의
  **판정**(PASS/FAIL·가용/비가용·"정본" 선언)을 뒤집으면, 뒤집는 문서를 쓰는 에이전트가 **선행 문서 헤더에
  1줄 배너를 추가**한다(과거 기록 위조가 아니라 주석 — plan_2026063021_1 배너 선례의 정형화):
  ```
  > ⚠ SUPERSEDED-IN-PART by `docs/<type>/<뒤집는 문서>.md` — <뒤집힌 판정 1줄>   (부분 반전)
  > ⛔ SUPERSEDED by `docs/<type>/<후속 문서>.md`                                  (문서 전체 대체)
  ```
  트리거는 **판정의 반전만**(보완·추가·상세화는 cites 로 충분 — 배너 남발 금지). wiki-desk 가 이 리터럴을
  결정론 grep 해 `superseded-by` 엣지로 색인하고 발현 시 경고를 병기한다(v1 결정론 규율 유지).
- **판정 어휘(앵커링 방지)**: 단정 판정에는 **유효맥락 한정자**를 병기한다 — 예 "cudagraph 비가용
  [맥락: 포크 c766cbc6 · GB10 · 2026-06-29 시점]". 무기한 어휘("정본"·"전역 금지")는 헌법/스킬 codify 를
  거친 것에만 허용(맥락-바운드 교훈을 전역화하지 않기 — 헌법 carry-forward 금지 따름정리의 문서 축).
- **후속 갱신 클로저**: "후속 확인 예정" 류 마커는 체크박스로 쓴다 — `- [ ] 후속: <무엇>`. 닫힐 때 해소
  문서 경로를 채워 `- [x] 후속: <무엇> → <해소 문서 경로>` 로 닫는다(영구 미결 잔존 방지).
- **PARKED 장부**: 파킹/미결 항목은 devlog "최종 상태" 섹션에 결정론 접두사 `- PARKED:` 로 표기
  (grep 한 방으로 전 미결 수집 — 별도 이슈트래커 없이 문서-분산 장부).
- **검증 게이트 정합**: 컨테이너 변경은 스모크 통과(testlog 증거) 전 done 금지. last-good 커밋 시
  `docs/devlog·testlog`에 버전·변경·스모크 결과·last-good를 기록(workflow S4).

## 4. 보관 / 전파 (브랜치 통합 모델)

- **작업 문서(`docs/<type>/*.md`)는 gitignore** → working-dir 단일 사본이 브랜치 전환에도 persist.
  따라서 `single-node`·`multi-node` 문서는 **자동 통합·동일**. (이 gitignore-persist는 문서 작업파일 전용 메커니즘이다.)
  구현체(`Dockerfile`·`docker-compose.yaml`·`configs/`·`envs/`)는 반대로 브랜치별 독립(통합 안 함) — 산출물은 `output/<topology>/`(single|multi) 통로에 두어 혼재 차단(CLAUDE.md "산출물 통로 불변식").
- **빌딩블럭(`.claude/`·`CLAUDE.md`)은 위와 다르다 — 이제 git-tracked**(배포 대상)이므로 브랜치 전환에 persist되지 않는다.
  브랜치 간 동일성은 `scripts/sync_branches.sh`로 **수동 동기화**해 유지한다(작업 종료 후 사람 질의).
- **추적·배포되는 것 = 폴더 스켈레톤 + 각 폴더 `example.md` 1개씩만**(역할+명명규칙 · **report/ 는 예외 —
  산출물째 추적**, 아래). 외부 배포 시 CLAUDE.md/.claude의 문서 규칙이 참조하는 폴더 구조가 항상 함께 존재하도록 보장.
  - `.gitignore`: `docs/*/*` (작업문서 무시) + `!docs/*/example.md` (스켈레톤만 추적).
  - **simlog·benchmark 도 동일 규칙으로 자동 처리**: 산출물(`docs/simlog/<run>/*` · `docs/benchmark/report_*.md`·
    `docs/benchmark/benchmark_*.yaml`)은 `docs/*/*`에 걸려 무시, `example.md`만 추적. **전용 추가 규칙 불필요**
    (별도 패턴 넣지 말 것 — 편지 A.2.1 "새 폴더마다 전용 gitignore 규칙 추가 금지"). 검증: `git add --dry-run docs/benchmark/` = example.md 만.
  - **⚠ report/ = 유일한 추적 예외**(`!docs/report/*`): 산출물째 추적·배포한다. 무시하면 클론에 안 실려
    **"배포자에게 알린다"는 목적이 성립 못 한다**(도달 0). 이는 **바로 위 "전용 규칙 추가 금지"의 명시
    예외** — 그 금지는 산출물을 *무시*하려는 폴더(simlog·benchmark)가 이미 `docs/*/*` 로 커버되니 중복을
    막는 것이고, report 는 **요구가 정반대(추적)** 라 전용 예외가 유일한 수단이다. **정합 위반으로 오인해
    제거 ✗.** 대가 = gitignore-persist 상실(브랜치 자동 통합 ✗) → `sync_branches.sh` 로 동기화(빌딩블럭 동형).
    검증: `git add --dry-run docs/report/` = html 포함 전부.
- 서브노드 **하향(빌드) rsync** 전파에서는 docs 제외(빌드 불필요 — `sync_to_sub.sh`의 `--exclude docs`). simlog의
  대용량 trial 로그도 이 제외로 서브에 하향 전파되지 않는다(빌드 입력 아님).
- 빌딩블럭(`.claude/`·`CLAUDE.md`)은 이 docs 규약의 대상 문서가 아니다(헌법·스킬이 관리). 추적·배포는 되며 브랜치 동기화는 `sync_branches.sh`. `seed/`는 비추적(사적 부트스트랩 이력).

### 서브노드 docs 테라포밍 + 상향 회수 (D12)

> 근거: `plan_2026062411_1`(D12). 절차 = `.claude/rules/workflow.md` §"메인↔서브 양방향 브랜치싱크" B2.

- **규약 테라포밍**: 서브노드도 **동일한 docs 발행 규약**(이 파일)을 따른다 — 같은 명명(`docs/<type>/<type>_YYYYMMDDHH_seq_주제.md`),
  같은 5종(plan/devlog/testlog/simlog/benchmark), 같은 gitignore-persist(`docs/*/*` ignore · `!docs/*/example.md` 추적). docs 스켈레톤은 `render_sub_env.py` 가 서브 env 에 렌더.
  **report/ 는 서브에 렌더하지 않는다 — 메인 전용**(배포자 대상 아웃바운드 공지 = 빌딩블럭 전파 축,
  `references.md` 동형). 서브 docs 는 *상향 insight 회수* 채널이라 성격이 다르다. 배제 배선 = `render_sub_env.py`
  의 `DOC_TYPES`(5종) 필터 — glob 이 6번째 폴더를 자동 흡수하지 않게 고정.
- **상향 회수(서브→메인) = 문서기반 only**: 서브가 자기개선 insight 를 자기 `docs/` 에 발행 → A2A 리포트로 **경로 전달** → 메인이
  `fetch_sub_docs.sh` 로 서브 `docs/` 만 로컬 gitignored 미러(`sync_staging/sub_docs/`)로 rsync → 메인 **열람** → **HITL 재저작**.
  (이는 하향 빌드 rsync 의 `--exclude docs` 와 별개 평면 — 빌드엔 docs 불요, **회수엔 docs 가 유일 채널**. patch/코드 추출 없음.)
- **PII**: 회수가 문서기반(코드/설정 미추출)이라 서브 헌법의 bake 정체성이 메인 추적물로 유입되지 않는다(헌법 §메인↔서브 D12-09).
