# docs.md — 문서 작성 규약 (plan · devlog · testlog · simlog)

> 이 파일은 워크스페이스의 **문서 4종 역할·명명·구조 규칙**이다. "항상 참인 사실"은 루트 `CLAUDE.md`,
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

## 2. 3종 역할 (무엇을 · 언제 · 어디에)

### plan/ — 계획서 (작업 **착수 전**)
- **역할**: 단계/Phase 작업의 계획·설계·접근방식. **사람 검토(HITL) 대상** — 실행 전 합의용.
- **시점**: 실행 이전. (부트스트랩: `docs/plan` 작성 → 사람 검토 → 실행 → 기록 → 다음.)
- **담는 것**: 목표·범위, 결정론적 해소값 설계, 단계별 절차, 리스크, 검증(합격) 기준.
- **두 무게, 한 장르 (plan_2026063018_1 · 로그=에이전트 철학)**: ① *스킬·헌법 개발*(우리) = **무거운** plan(전 섹션). ② **배포자 온보딩**(`terraforming_node` init-mode)도 **같은 `docs/plan/` 장르**로 발행한다(경량 별도장르 ✗ — wiki-desk 색인·습관화 = 자기개선 루프 수동 기둥 첫 접점). 다만 비전공 배포자 부담을 줄이기 위해 **에이전트가 인터뷰 답에서 대신 초안**(토폴로지·획득모드·스캔할일·branch정합·완료 시 Flag)하고 **배포자가 자기-HITL 승인** — *무게가 아니라 경험*을 매끄럽게(약간의 강요는 의도된 철학). 온보딩 plan 의 실제 깊이는 작업 복잡도에 비례(단일노드 스캔이면 짧음).

### devlog/ — 작업 로그 (작업 **중·후**)
- **역할**: 실제 수행한 **작업 내역·결정·전파의 서사**("무엇을 했나").
- **담는 것**: 작업 흐름, 핵심 사건/결정(+근거), 교훈, **최종 상태**(미커밋·다음 작업 명시).

### testlog/ — 검증 로그 (검증 **결과·증거**)
- **역할**: 빌드/스모크/실험의 **증거와 판정**("동작을 확인했나").
- **담는 것**: 목적, 전제(resolve값), 실행 커맨드, 관측(로그·수치), **합격/실패 판정**, 실패 시 근본원인.

### simlog/ — 시뮬레이션 증거 vault (`recipe.py simulate` 산출)
- **역할**: VRAM 시뮬레이터 trial-loop **한 run의 원시 증거 적재함**("실측이 정확히 무엇이었나").
  testlog가 사람용 종합 보고서라면, simlog는 그 보고서가 인용하는 **기계 생성 raw 증거**다.
- **구조**: 파일 1개가 아니라 **run 디렉토리 1개**(`simlog_writer.py`가 기록). 한 run = candidate set의
  trial 반복 + 조정 이력 + 최종 요약. 폴더 내용(trial NN은 `01`부터):
  - `trialNN_vllm.log` — 컨테이너 docker logs 원문(`VLLM_LOGGING_CONFIG_PATH`로 `/app/simlog`에 캡처).
  - `trialNN_profile.json` — `parse_vllm_log` 실측(weights/kv/overhead GiB, 백엔드 등).
  - `trialNN_candidate.yaml` — 그 trial의 lock-set + 설정한 `kv_cache_memory_bytes`.
  - `trialNN_smoke.json` — `functional_smoke` 결과(completion/tool_call/reasoning).
  - `correction_history.jsonl` — trial 간 결정론 조정·soft 변수 폴백 1줄/건.
  - `run_summary.json` — 수렴 여부·trial_count·최종 채택 candidate·실측 분해.
- **vLLM 로깅**: 컨테이너에 `vllm_logging_config.json`(simlog_writer 템플릿)을 `VLLM_LOGGING_CONFIG_PATH`로
  주입해 로그를 simlog 경로 파일핸들러로 떨군다.

## 3. 작성 원칙

- **분리**: 한 작업의 *서사*는 devlog, 그 *검증 증거/판정*은 testlog로 분리. 계획은 plan.
  (검증을 동반한 작업이면 devlog + testlog 한 쌍이 보통.)
- **적용 범위**: plan은 단계/Phase 등 큰 작업에 작성(소규모 작업은 생략 가능). devlog는 의미 있는
  작업마다 남긴다. testlog는 빌드/스모크/검증을 수행했을 때 남긴다.
- **상호 참조**: 본문에 관련 문서 경로를 명기(devlog → 해당 testlog, plan → Seed/근거 등).
- **참조 체인**: `simlog`(원시 증거) → `testlog`(인용·종합 보고서·판정) → `devlog`(서사). simlog는 사람이
  직접 읽기보다 testlog가 경로로 인용하는 증거 저장소다. (simulate run이면 testlog 본문에 simlog run 경로 명기.)
- **사실 우선**: 절대 날짜·결정론적 값(torch 핀·NGC 태그·스모크 결과)을 명시. 추측은 "확인 필요"로 표기.
- **검증 게이트 정합**: 컨테이너 변경은 스모크 통과(testlog 증거) 전 done 금지. last-good 커밋 시
  `docs/devlog·testlog`에 버전·변경·스모크 결과·last-good를 기록(workflow S4).

## 4. 보관 / 전파 (브랜치 통합 모델)

- **작업 문서(`docs/<type>/*.md`)는 gitignore** → working-dir 단일 사본이 브랜치 전환에도 persist.
  따라서 `single-node`·`multi-node` 문서는 **자동 통합·동일**. (이 gitignore-persist는 문서 작업파일 전용 메커니즘이다.)
  구현체(`Dockerfile`·`docker-compose.yaml`·`configs/`·`envs/`)는 반대로 브랜치별 독립(통합 안 함) — 산출물은 `output/<topology>/`(single|multi) 통로에 두어 혼재 차단(CLAUDE.md "산출물 통로 불변식").
- **빌딩블럭(`.claude/`·`CLAUDE.md`)은 위와 다르다 — 이제 git-tracked**(배포 대상)이므로 브랜치 전환에 persist되지 않는다.
  브랜치 간 동일성은 `scripts/sync_branches.sh`로 **수동 동기화**해 유지한다(작업 종료 후 사람 질의).
- **추적·배포되는 것 = 폴더 스켈레톤 + 각 폴더 `example.md` 1개씩만**(역할+명명규칙). 외부 배포 시
  CLAUDE.md/.claude의 문서 규칙이 참조하는 폴더 구조가 항상 함께 존재하도록 보장.
  - `.gitignore`: `docs/*/*` (작업문서 무시) + `!docs/*/example.md` (스켈레톤만 추적).
  - **simlog도 동일 규칙으로 자동 처리**: run 디렉토리 전 산출물(`docs/simlog/<run>/*`)은 `docs/*/*`에
    걸려 무시, `docs/simlog/example.md`만 추적. simlog 전용 추가 규칙 불필요(별도 패턴 넣지 말 것).
- 서브노드 **하향(빌드) rsync** 전파에서는 docs 제외(빌드 불필요 — `sync_to_sub.sh`의 `--exclude docs`). simlog의
  대용량 trial 로그도 이 제외로 서브에 하향 전파되지 않는다(빌드 입력 아님).
- 빌딩블럭(`.claude/`·`CLAUDE.md`)은 이 docs 규약의 대상 문서가 아니다(헌법·스킬이 관리). 추적·배포는 되며 브랜치 동기화는 `sync_branches.sh`. `seed/`는 비추적(사적 부트스트랩 이력).

### 서브노드 docs 테라포밍 + 상향 회수 (D12)

> 근거: `seed_e34dfbb6ec23` · `plan_2026062411_1`. 절차 = `.claude/rules/workflow.md` §"메인↔서브 양방향 브랜치싱크" B2.

- **규약 테라포밍**: 서브노드도 **동일한 docs 발행 규약**(이 파일)을 따른다 — 같은 명명(`docs/<type>/<type>_YYYYMMDDHH_seq_주제.md`),
  같은 4종(plan/devlog/testlog/simlog), 같은 gitignore-persist(`docs/*/*` ignore · `!docs/*/example.md` 추적). docs 스켈레톤은 `render_sub_env.py` 가 서브 env 에 렌더.
- **상향 회수(서브→메인) = 문서기반 only**: 서브가 자기개선 insight 를 자기 `docs/` 에 발행 → A2A 리포트로 **경로 전달** → 메인이
  `fetch_sub_docs.sh` 로 서브 `docs/` 만 로컬 gitignored 미러(`sync_staging/sub_docs/`)로 rsync → 메인 **열람** → **HITL 재저작**.
  (이는 하향 빌드 rsync 의 `--exclude docs` 와 별개 평면 — 빌드엔 docs 불요, **회수엔 docs 가 유일 채널**. patch/코드 추출 없음.)
- **PII**: 회수가 문서기반(코드/설정 미추출)이라 서브 헌법의 bake 정체성이 메인 추적물로 유입되지 않는다(헌법 §메인↔서브 D12-09).
