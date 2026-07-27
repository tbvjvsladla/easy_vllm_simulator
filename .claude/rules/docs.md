# docs.md — 문서 발행 계약

> 목표: 계획·작업·검증·계측·공지를 역할별로 분리하고 재현 가능한 이름과 evidence chain으로 남긴다.
> 상세 생성/검증은 owner 스킬과 `scripts/doc_naming.py`·`scripts/evidence_publisher.py`가 소유한다.

## 명명 SSOT

- 기본: `docs/<type>/<type>_<YYMMDDHH>[_<MM>_<SS>]_<주제>.md` (`type`=`plan|devlog|testlog`). `YYMMDDHH`는 KST 2자리 연도 절대시각이다.
- **충돌 시에만 `_MM_SS`**를 붙인다. 같은 type/hour의 기본형과 해당 충돌형이 점유되면 새 형식/덮어쓰기 없이 `NamingCollisionExhausted`로 fail-closed한다.
- 주제는 간결한 한국어 `_` slug. 평면 `docs/파일.md`, 상대날짜, `_seq_`는 금지한다.
- simlog: `docs/simlog/<YYMMDDHH>[_<MM>_<SS>]_<주제>/` (run 디렉터리).
- benchmark: `docs/benchmark/bench_report_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md`는 항상, `docs/benchmark/benchmark_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.yaml`은 PASS 때만 발행한다. report prefix 정본은 `bench_report_`; timestamp/collision은 `scripts/doc_naming.py`.
- outbound report: `docs/report/<kebab-case-topic>.<html|md>`; 날짜 없이 최신본을 갱신한다.

## compact document matrix

| 종류 | 목표/상태 | owner·입력 | 출력 | gate | 실패 라우팅 |
|---|---|---|---|---|---|
| `plan/` | 실행 전 intent | 작업 owner·인터뷰/범위 | 목표·범위·단계·risk·합격기준 | 실행 전 HITL | 범위/승인 보완 |
| `devlog/` | 작업 중·후 narrative | 작업 owner·실행 사건 | 결정·시도/폐기·최종상태·재개지침 | 사실/경로 확인 | `PARKED:`·후속 checkbox |
| `testlog/` | evidence verdict | 검증 owner·resolve/config/log | command·관측·환경 snapshot·PASS/FAIL | 실제 smoke/검증 | root cause+owner route |
| `simlog/` | raw trial vault | recipe/smoke scripts·per-trial facts | log/profile/candidate/smoke/history/summary | run 완결성 | testlog에서 누락 명시 |
| `benchmark/` | full 계측 | `adversarial-benchmark`·측정값 | 항상 report, PASS만 flat certificate | verdict owner·재현성 | FAIL report만; 합성 금지 |
| `report/` | 배포자 공지 | 사람·공지 본문 | self-contained HTML/MD 최신본 | PII·배포 검토 | 자동발행/위키색인/서브전파 금지 |

## 공통 발행 계약

- 역할 분리: plan=intent, devlog=서사, testlog=판정, simlog=원시 trial, benchmark=inform-only 계측, report=outbound 공지.
- evidence chain: simlog/raw → benchmark report → testlog verdict → devlog narrative. 관련 경로를 본문에 기록하고, 가변 파일의 line 번호에는 literal 또는 commit SHA를 병기한다.
- 판정은 모델/HW/version/date 유효맥락을 붙인다. 후속은 `- [ ] 후속:`에서 시작해 해소 문서로 `- [x]` 닫는다.
- 후속 문서가 기존 판정을 뒤집을 때만 선행 헤더에 `SUPERSEDED-IN-PART` 또는 `SUPERSEDED`와 후속 경로를 기록한다. 단순 보완은 citation만 추가한다.
- 미완결 devlog는 디스크/노드 상태, 대기 결정, 재개 command, 예상 벽을 담는다. testlog는 당시 image/env/config를 복원 가능하게 하고 per-trial config는 simlog에 보존한다.

## 보관·전파 matrix

| 경로 | Git 상태 | main/sub 전파 | owner/gate |
|---|---|---|---|
| `docs/{plan,devlog,testlog,simlog,benchmark}/*` | 작업 산출물 ignored; `example.md`만 tracked | 브랜치 전환에 working copy 유지; build rsync 제외 | `.gitignore`의 `docs/*/*` + `!docs/*/example.md` |
| `docs/report/*` | **유일한 tracked docs 산출물 예외** | main-only; branch sync 대상 | `!docs/report/*`, PII gate |
| `.claude/`·`CLAUDE.md` | tracked building blocks | `scripts/sync_branches.sh` | 이 문서 산출물 규약 밖 |
| `seed/` | private/untracked | 배포본에 없을 수 있음 | 근거 pointer만 허용 |

simlog·benchmark에 폴더별 ignore 예외를 더하지 않는다. report는 tracked allowlist 행 하나로 평탄화하며, `docs/report/` 전체가 배포된다.

## 서브 docs 계약

서브에는 plan/devlog/testlog/simlog/benchmark 다섯 skeleton만 렌더하고 report는 렌더하지 않는다. **상향 회수(서브→메인) = 문서기반 only**: 서브 발행→A2A path 전달→`fetch_sub_docs.sh`가 docs만 ignored mirror로 회수→메인이 열람/HITL 재저작한다. 코드·설정 patch 직접 회수와 서브 재스캔은 금지한다.

## publisher 계약

| command | 입력→출력 | fail-closed gate |
|---|---|---|
| `init` | task class→deterministic scaffold/publication record | 기존 narrative/raw 비덮어쓰기 |
| `append-raw` | repo-relative regular UTF-8 evidence→append-only entry | absolute/escape/symlink/FIFO/empty/wrong type 거부 |
| `set-narrative` | 명시 narrative file→provenance-bound marker | publisher 산문 합성 금지 |
| `publish-benchmark` | 실제 full result→report/certificate | FAIL certificate·누락 certificate 합성 금지 |
| `record-capacity-rejection` | 검증된 gate pointer→record | fabricated evidence 금지 |
| `finalize` | record→work manifest→completion gate | identity/PII/verdict 자체판정 금지 |

시각은 `--generated-utc`/`--recorded-utc` 입력만 사용한다. required evidence와 최종 상태는 각각 `completion_gate.required_evidence_for()`와 `completion_gate.py verify`가 소유한다.
