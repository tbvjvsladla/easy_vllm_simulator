# docs.md — 문서 발행 계약

> 목표: 계획·작업·검증·계측·공지를 역할별로 분리하고 재현 가능한 이름과 evidence chain으로 남긴다.
> 상세 생성/검증은 owner 스킬의 `.claude/skills/wiki-desk/scripts/doc_naming.py`와 헌법 runtime의 `.claude/policies/runtime/evidence_publisher.py`가 소유한다.

## 명명 SSOT

- 기본: `docs/<type>/<type>_<YYMMDDHH>[_<MM>_<SS>]_<주제>.md` (`type`=`plan|devlog|testlog|request|checklist`). `YYMMDDHH`는 KST 2자리 연도 절대시각이다.
- **충돌 시에만 `_MM_SS`**를 붙인다. 같은 type/hour의 기본형과 해당 충돌형이 점유되면 새 형식/덮어쓰기 없이 `NamingCollisionExhausted`로 fail-closed한다.
- 주제는 간결한 한국어 `_` slug. 평면 `docs/파일.md`, 상대날짜, `_seq_`는 금지한다.
- simlog: `docs/simlog/<YYMMDDHH>[_<MM>_<SS>]_<주제>/` (run 디렉터리).
- benchmark: `docs/benchmark/bench_report_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md`는 항상, `docs/benchmark/benchmark_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.yaml`은 PASS 때만 발행한다. report prefix 정본은 `bench_report_`; timestamp/collision은 `.claude/skills/wiki-desk/scripts/doc_naming.py`.
- outbound report: `docs/report/<kebab-case-topic>.<html|md>`; 날짜 없이 최신본을 갱신한다.
- **request(수행지시서)**: `docs/request/request_<YYMMDDHH>[_MM_SS]_<주제>.md`. 기본형과 동일한 날짜 규약을
  쓴다 — report 와 달리 "최신본 갱신"이 아니라 **발행 시점이 고정된 작업지시**이기 때문이다(같은 과업을
  다시 지시하면 새 문서를 낸다). 발행 대상은 **에이전트가 닿을 수 없는 평면에서 사람이 직접 수행할 절차**다.
- **`docs/logs/` 는 이 명명 SSOT의 명시 예외다** — 문서가 아니라 기계판독 데이터 평면이므로
  `<type>_<YYMMDDHH>_<주제>` 규약·산문 규약을 적용하지 않는다(아래 §기계판독 데이터 평면).

## compact document matrix

| 종류 | 목표/상태 | owner·입력 | 출력 | gate | 실패 라우팅 |
|---|---|---|---|---|---|
| `plan/` | 실행 전 intent | 작업 owner·인터뷰/범위 | 목표·범위·단계·risk·합격기준 | 실행 전 HITL | 범위/승인 보완 |
| `devlog/` | 작업 중·후 narrative | 작업 owner·실행 사건 | 결정·시도/폐기·최종상태·재개지침 | 사실/경로 확인 | `PARKED:`·후속 checkbox |
| `testlog/` | evidence verdict | 검증 owner·resolve/config/log | command·관측·환경 snapshot·PASS/FAIL | 실제 smoke/검증 | root cause+owner route |
| `simlog/` | raw trial vault | recipe/smoke scripts·per-trial facts | log/profile/candidate/smoke/history/summary | run 완결성 | testlog에서 누락 명시 |
| `benchmark/` | full 계측 | `adversarial-benchmark`·측정값 | 항상 report, PASS만 flat certificate | verdict owner·재현성 | FAIL report만; 합성 금지 |
| `report/` | 배포자 공지 | 사람·공지 본문 | self-contained HTML/MD 최신본 | PII·배포 검토 | 자동발행/위키색인/서브전파 금지 |
| `request/` | **사람 수행 지시** | 에이전트·범위/전제/한계 | 전제→절차→검증→회수물 순의 실행가능 매뉴얼 | 절차가 실제 실행가능한지(버전·명령 핀) | 수행자 피드백→개정 발행 |
| `checklist/` | bot 전용 단계 트래킹 | 검증 owner·3-Phase(Planner/Builder/Validator) 상태 | phase별 항목 체크 + 재개지침 | 최신 snapshot(YYMMDDHH) 기준 | 최신 체크리스트로 복귀 |

### `request/` — 에이전트가 못 닿는 평면을 사람에게 위임하는 문서 (7번째 산문형)

에이전트의 실행 평면 밖(다른 아키텍처의 머신, 물리 작업, 외부 계정 권한)에서만 완수 가능한 과업이
드러났을 때, **그 과업을 사람이 재현 가능하게 수행하도록** 발행한다. plan 이 "우리가 무엇을 할 것인가"라면
request 는 "당신이 무엇을 어떻게 해야 하는가"다.

- **필수 구성**: ① 전제·준비물(하드웨어·네트워크·권한) ② 단계별 명령(복붙 가능·버전 핀) ③ 각 단계의
  **성공 판정 기준** ④ 실패 시 분기 ⑤ **회수물 목록**(수행 후 에이전트에게 돌려줄 파일) ⑥ 예상 소요·비용.
- **금지**: 추측 명령(실행해 보지 않은 절차를 검증된 것처럼 적기), 회수물 없는 지시(수행 결과가
  에이전트로 돌아오지 못하면 미완결). 둘 다 문서의 **정직성·완결성**에 걸린 것이라 상황 예외가 없다.
- **자제**: 환경 구체값 하드코딩(주소·경로는 플레이스홀더 + 획득 방법을 적는 것이 기본). 다만 이는
  4종 안티패턴이므로 **금지가 아니라 자제**다(§결정론 규율 판정표) — 수행자가 실제로 그 값을 못 얻는
  상황이면 구체값을 적고 **출처와 유효 범위를 함께** 밝히는 편이 낫다. 플레이스홀더 원칙을 지키느라
  실행 불가능한 지시서를 내는 것이 더 큰 결함이다.
- **evidence chain 밖이다** — request 는 판정도 계측도 아니므로 `evidence_publisher` 의 완료 게이트를
  타지 않는다. 수행 결과가 돌아오면 그때 testlog/devlog 로 chain 에 편입한다.
- **비추적**(아래 §보관·전파 matrix). 운영자 환경 절차라 배포 대상이 아니다.

## 기계판독 데이터 평면 — `docs/logs/` (8번째, 유일한 비-문서)

> 근거 `plan_26073109`(노드블랙박스 승격) · 소유 `terraforming_node/scripts/node_blackbox/`.
> **사람 가독성을 고려하지 않는다**(사용자 결정) — 열람이 필요하면 그때 비패턴 업무로 md/html 변환한다.
> 산문 7종과 성격이 다르므로 §명명 SSOT·§공통 발행 계약·evidence chain 규약을 적용하지 않는다.

| 경로 | 내용 | 포맷 근거 | 수명 |
|---|---|---|---|
| `docs/logs/<node_id>/samples/<YYYY-MM-DD>.csv` | 1초 원시 시계열 | 키 반복이 없어 JSONL 대비 약 1/3 용량 | 7일 → 압축 30일 → 삭제 |
| `docs/logs/<node_id>/events/<YYYY-MM>.jsonl` | 희소·이질 이벤트(트립·킬·부정클린부팅) | 자기서술 필요, 양이 적음 | **영구** |
| `docs/logs/<node_id>/rollup/<YYYY-MM-DD>.json` | 일별 포락선 통계 | 학습의 실제 입력 | **영구** |
| `docs/logs/<node_id>/envelope.json` | 현재 포락선 + ETA 상수 | **에이전트가 폴링마다 읽는 유일한 파일**(수백 토큰) | 갱신 |
| `docs/logs/<node_id>/capture_verified.json` | proof-of-capture 판정 | 상태 권위(`installed` 아님) | 갱신 |
| `docs/logs/<node_id>/seed/` | 레거시 저널 수확분 | 15초 해상도 재구성(canonical 아님) | 보존 |

- **수명 집행 순서가 곧 안전장치다**: rollup(통계 확정) → 압축 → 나이삭제 → 용량삭제. 원시를 버려도
  학습 입력은 남는다. 노드당 총량 상한(기본 512 MiB) 초과 시 **오래된 samples 부터** 삭제한다.
- **침묵 삭제 금지**: 모든 삭제·압축은 `events` 에 `log_evicted`/`log_compressed` 로 남긴다 —
  조용한 삭제는 "기록이 원래 없었던 것"과 구분되지 않는다.
- **부재와 결측의 구분**: GB10 통합메모리는 GPU 메모리 지표가 존재하지 않으므로(`nvidia-smi
  memory.used` = `[N/A]`) `gpu_mem` 열은 상시 빈 칸이며 이는 정상이다.
- 시각은 `--now` 주입만 사용한다(벽시계 금지 — `staleness_gate.py --max-age-days` 선례 정합).

## PII 스캔 적용 범위 (2026-07-31 확정)

정본 패턴은 `.claude/skills/hint-publisher/scripts/hint_tag.py` 의 `GENERIC_PII` 4종
(`private-ipv4`·`email`·`abs-op-path`·`spark-host`) + `.claude/pii_terms.txt` 리터럴이다.
**적용 강도는 산출물이 배포되는지로 갈린다** — 배포되지 않는 것에 배포 기준을 적용하면 게이트가
과잉차단되고, 배포되는 것에 완화 기준을 적용하면 유출된다.

**적용 강도와 별개로 판정 *대상* 이 갈린다** — 사람이 저작하는 평면만 판정한다. 기계가 생성하고
**편집하지 않는 것이 계약**인 평면은 판정해도 처방이 없다(2026-08-15 개정 · `plan_26081516` H3).

| 대상 | 판정 여부 | 적용 패턴 | 근거 |
|---|---|---|---|
| **배포 산출물** — hint 태그 오브젝트·`docs/report/*`·추적 템플릿(`.claude/**`·`CLAUDE.md`)·서브 전파분 | **판정** | **4종 전부**(`abs-op-path` 포함) | 제3자에게 도달한다. 운영자 절대경로는 환경 지문이므로 제거 대상 |
| **비배포 산문** — gitignored `docs/{plan,devlog,testlog,benchmark,request}` (사람 저작분) | **판정** | `private-ipv4`·`email`·`spark-host` **3종** | 로컬 전용. `/mnt`·`/home` 경로는 재현에 필요한 정보이며 배포되지 않는다 |
| **기계생성 원시 평면** — `docs/simlog/*`(trial vault)·`docs/logs/*`(블랙박스 데이터) | **판정 대상 밖** | — | **편집 불가가 계약**이다(§compact document matrix `simlog`=raw trial vault · §기계판독 데이터 평면). 소멸은 정정이 아니라 **수명주기 삭제**로만 일어난다 |

- **판정 대상 축소의 근거**(2026-08-15 실측 · `testlog_26081516` §4.1): `spark-host` 전수 251,982건 중
  **99.97%가 `docs/simlog`·`docs/logs`** 였다. 이 둘을 포함한 채로 "0건"을 기준으로 삼으면 그 기준은
  **정의상 달성 불가**하며, 달성하려면 원시 vault 편집 금지 계약을 깨야 한다. 아래 Docker 브리지
  예외가 이미 같은 논리를 폈다 — *로그를 고치는 대신 판정에 예외를 둔다.*
- 판정 대상 밖이라고 **유출 위험이 없다는 뜻은 아니다** — 그 평면은 애초에 비추적·비배포이고
  (`.gitignore` `docs/*/*`), 상향 회수도 문서기반이라 제3자에게 도달하는 경로가 없다. 위험이 없는 것이
  아니라 **처방이 정정이 아니라 격리·수명주기**인 것이다.

- `abs-op-path` 를 비배포 문서까지 확대하면 `nas_model_path`(manifest 정규 필드)를 인용한 모든
  계획·판정 문서가 비준수가 된다 — 정보를 잃는 대신 얻는 안전이 없다.
- work-manifest 의 `pii_scan.passed` 는 **위 표의 해당 강도로 실제 스캔한 결과**만 적는다.
  좁은 패턴으로 스캔하고 통과를 선언하면 그 선언 자체가 거짓이다(2026-07-31 실제 발생).
- `hint_tag finalize`/`verify` 의 fail-closed 스캔은 이 완화와 **무관하게 4종 전부**를 강제한다 —
  배포 경로의 최종 권위는 그쪽이다.
- **Docker 기본 브리지 예외(비배포 원시로그 한정)**: 엔진 로그에 나오는 Docker 고정 기본 `docker0`
  서브넷(`172.17.x.x/16`)은 모든 Docker 호스트에 동일하게 존재하며 운영자 네트워크를
  식별하지 않는다. `docs/simlog/*`·`docs/logs/*` 의 **기계생성 원시로그**에 한해 `private-ipv4`
  매치에서 제외한다. **배포 산출물에는 적용하지 않는다** — 거기서는 4종 전부가 그대로 강제된다.
  (원시 vault 는 편집하지 않는 것이 계약이므로, 로그를 고치는 대신 판정에 예외를 둔다.)
  이 문장 자체는 배포되는 추적 파일이므로 대역을 **리터럴로 적지 않는다** — 규칙 문서가 자기 스캔에
  걸리면 게이트가 무의미해진다(2026-08-06 실제 발생: 이 줄이 4종 스캔의 유일한 위반이었다).

## 공통 발행 계약

- 역할 분리: plan=intent, devlog=서사, testlog=판정, simlog=원시 trial, benchmark=inform-only 계측, report=outbound 공지, request=사람 수행지시, checklist=bot 전용 단계 트래킹(최신 snapshot 기준).
- evidence chain: simlog/raw → benchmark report → testlog verdict → devlog narrative. 관련 경로를 본문에 기록하고, 가변 파일의 line 번호에는 literal 또는 commit SHA를 병기한다.
- 판정은 모델/HW/version/date 유효맥락을 붙인다. 후속은 `- [ ] 후속:`에서 시작해 해소 문서로 `- [x]` 닫는다.
- 후속 문서가 기존 판정을 뒤집을 때만 선행 헤더에 `SUPERSEDED-IN-PART` 또는 `SUPERSEDED`와 후속 경로를 기록한다. 단순 보완은 citation만 추가한다.
- 미완결 devlog는 디스크/노드 상태, 대기 결정, 재개 command, 예상 벽을 담는다. testlog는 당시 image/env/config를 복원 가능하게 하고 per-trial config는 simlog에 보존한다.

## 보관·전파 matrix

| 경로 | Git 상태 | main/sub 전파 | owner/gate |
|---|---|---|---|
| `docs/{plan,devlog,testlog,simlog,benchmark,request,checklist}/*` | 작업 산출물 ignored; `example.md`만 tracked | 브랜치 전환에 working copy 유지; build rsync 제외 | `.gitignore`의 `docs/*/*` + `!docs/*/example.md` |
| `docs/report/*` | **유일한 tracked docs 산출물 예외** | main-only; branch sync 대상 | `!docs/report/*`, PII gate |
| `docs/logs/*` | ignored(기존 `docs/*/*` 가 이미 커버 — 새 규칙 불요) | **평시 `envelope.json` 요약만 상향**, 사고 시에만 원시 회수 | `logs_lifecycle.py`; `example.md` 스켈레톤 불요(기계 생성) |
| `.claude/`·`CLAUDE.md` | tracked building blocks | `.claude/skills/upstream-version-watch/scripts/sync_branches.sh` | 이 문서 산출물 규약 밖 |
| `seed/` | private/untracked | 배포본에 없을 수 있음 | 근거 pointer만 허용 |

simlog·benchmark에 폴더별 ignore 예외를 더하지 않는다. report는 tracked allowlist 행 하나로 평탄화하며, `docs/report/` 전체가 배포된다.

## 서브 docs 계약

서브에는 plan/devlog/testlog/simlog/benchmark 다섯 skeleton만 렌더하고 report·request·checklist는 렌더하지 않는다(셋 다 메인 전용 — report는 배포자 대상, request는 운영자 수행 지시, checklist는 bot 전용 감사 트래킹이라 서브가 발행할 주체가 아니다). **상향 회수(서브→메인) = 문서기반 only**: 서브 발행→A2A path 전달→`fetch_sub_docs.sh`가 docs만 ignored mirror로 회수→메인이 열람/HITL 재저작한다. 코드·설정 patch 직접 회수와 서브 재스캔은 금지한다.

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
