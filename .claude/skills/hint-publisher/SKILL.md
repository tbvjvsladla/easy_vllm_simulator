---
name: hint-publisher
description: 검증된 서빙 지식을 **셀 1개 = 태그 1개**로 `hint/<vllm>/<model>/<arch>/<recipe>` 태그에 **저작·발행·배포**한다. 이름은 도구가 전량 파생하고, 태그는 hint 전용 브랜치의 페이로드 커밋을 가리키므로 태그의 zip 이 곧 지도·서사·재현 키트다. "hint 태그 발행", "힌트 내보내기", "이 셀 발행하자", "이 레시피 배포하자", "카탈로그 갱신", "비슷한 hint 찾아줘" 같은 지시에 발동. 수신자(배포받은 코드에이전트)의 토큰노믹스가 목적이며, 저작은 메인 단독·카탈로그는 원격 발행 태그에서 파생한다.
---

# hint-publisher — 한 셀의 여정을 수신자가 다시 걷지 않게

> 정본 계약: `hints/HINT_ISSUANCE_CONTRACT.md`(v6 — 규칙과 그 이유 · 차단 code 표 · 페이로드 형식) ·
> 단일 진입: `scripts/hint.py`(명령별 인자 = `hint.py <명령> --help` · 불변식 = 모듈 docstring) ·
> 설계: `docs/plan/plan_26092119`(§3 결정 · §4 설계) · 발동 게이트: `policy:HINT_TAG_ACTIVATION_GATE`
> 이 문서는 **언제 · 무엇을 · 어떤 순서로** 만 적는다. 규칙의 세부는 계약 절 번호로 가리킨다.

## 0. 무엇을 위한 것인가

배포받은 다른 코드에이전트가 **이미 뚫린 벽을 다시 탐색하지 않게** 하는 자료다 — 에이전트 작업의 입력 토큰은 대부분 탐색이 먹으므로,
벽의 순서와 값의 지위를 넘겨주는 것이 가장 큰 절약이다. 완제품이 아니라 **여정의 지도**다(계약 §1).

**2026-09-21 재구성**(plan_26092119): 옛 발행기는 "무엇이 있는가" 만 검사하고 "무엇을 적어야 하는가" 를 구조로 갖지 않았다 — 한 태그는
zip 바이트의 25.6% 만 유효했고 결정적 핀은 zip 밖 태그 본문에만 있었으며, 다른 태그는 알려진 여정의 43% 만 싣고 사람이 재저작한 사실
3건이 틀렸다. 그래서 다섯을 바꿨다: ① 계보 전체를 읽고 챕터별 기재 지시(PROMPT)를 채우는 서사 ② 실제로 쓰인 산출물만 ③ 도구가
전량 파생하는 이름 ④ 셀 1개 = 태그 1개의 단일 진입 ⑤ 이번 발행분만 보는 게이트. 과거 태그는 **건드리지 않는다**(리콜 금지).

**상태를 전이하지 않는다** — workflow.md 의 spine 밖에서, 다른 스킬이 만든 관측을 읽는다:

| 발행이 읽는 것 | 만드는 주체 |
|---|---|
| 서빙 관측(멀티 스모크의 `serve_proof_<cell>.json` · 스윕 `post_health` + 벤치 completed · lite raw/warm) | upstream `multinode_serve_smoke.sh` · `adversarial-benchmark` |
| 트리플렛 · 런타임 패치(serve 시점 성립분) | `vllm-recipe-explorer` |
| 빌드 레시피 · 빌드 패치 · 이미지 안 빌드 원장 · 서브 전달 목록(`slave_forward.py`) | `upstream-version-watch` |
| 측정(인증서 · bench_report · 스윕 · lite) | `adversarial-benchmark` |
| 셀 문서(testlog · devlog)와 증거 포인터 | 작업 owner · `campaign_init.py --evidence-add` |
| 셀별 발행 사전승인(`hint_targets[].approval`) | `terraforming_node` `campaign_init.py --hint-approve` |

## 1. 발동 — 셀 단위 · 제안(Y/N)

**그 셀의 서빙·측정·문서 완료 후** — 셀 하나가 서빙(health 200 + 추론 1회)·측정(인증서 또는 bench_report)·문서(testlog·devlog) 발행을
마친 뒤에만 그 셀의 hint 태그 발행을 **제안(Y/N)** 한다(**무인 자동 태깅 ✗**). 캠페인 셀은 선언 확인 팝업에서 승인된
`hint_targets[].approval`(`campaign_init.py --hint-approve`)이 그 Y/N 의 사전 기록이고, 캠페인 밖 발행은
`hint.py continue --approved-by/--approved-utc` 전사가 그 기록이다. 승인된 Y/N 뒤의 태그 push 는 에이전트 상시승인(2026-08-24)이고,
브랜치 push(hint 브랜치 포함)는 발행에 속하지 않는다.

- 발행은 캠페인 완주와 분리돼 있다(2026-09-08) — 셀이 끝나면 그 셀을 낸다. 제안 트리거는 recipe closer(`vllm-recipe-explorer`
  `references/serving-closeout.md` ⑤)와 bump closer(`upstream-version-watch` §hint)가 든다. 파생 이름이 이미 원격에 있으면 새 태그가 아니다.
- `approved_by` 는 페이로드로 복사된다 — 이름·연락처가 아니라 **역할과 발화 요지**를 전사한다.

## 2. 한 셀 발행 — publish → 저작 → 독립 사실 검증 → continue

```bash
H=.claude/skills/hint-publisher/scripts/hint.py
python3 $H name     --campaign <id> --cell <cell>                        # (선택) 파생 이름 · 축별 출처 미리보기 · 읽기 전용
python3 $H publish  --campaign <id> --cell <cell> --generated-utc <UTC>  # 증거 → 이미지 탐침(기본) → 스캐폴드 → **정지**
#   ── 저작: draft 의 `<<AGENT:` 줄을 PROMPT 지시대로 산문 · hint-event · 원문 발췌로 바꾼다 ──
python3 $H excerpt  --draft <draft> --source <문서 stem|경로|artifacts 파일명|<도구>@<rev12>> [--lines a-b] [--numbered]  # 정규화 · 치환 후 원문(그대로 옮긴다 · --numbered = §L 번호)
python3 $H refresh  --draft <draft>                                        # (선택) 00 §0.3 · §0.5 파생 요약을 draft 에 미리 채워 본다
python3 $H lint     --draft <draft>                                        # 부수효과 0 · 반복 확인(0 이 될 때까지)
#   ── 독립 사실 검증: 저작자가 **아닌** Agent 가 <draft>/inputs/factcheck.json 을 쓴다 → 저작자가 고치고 lint 0 ──
python3 $H continue --campaign <id> --cell <cell> --generated-utc <UTC>  # 승인 → 린트 → 사실 검증 게이트 → 커밋 → 봉인 → push → 카탈로그
```

- **publish** 는 셀 증거(명시 id 만 · `campaigns/ACTIVE` 를 읽지 않는다) → 발행 자격 **관측**(계약 §2) → 이름 파생 · 충돌 확인(§4) →
  발행기 구동과 게이트 사전 확인 → 계보(`LINEAGE.json`) → 적용된 산출물만 수집(§3 A) → 사실 블록을 채운 스캐폴드 → **정지**한다.
  태그 · 브랜치 부수효과 0. 출력은 채울 PROMPT 목록 · 계보 읽기 목록(크기 포함) · 다음 명령이다. draft 는 `hints/.drafts/<draft_id>/`
  (gitignored — `payload/` · `state.json` · 도구가 쓴 `inputs/` — 측정 도구 원문 스냅샷 `inputs/sources/<도구>@<rev12>` 포함). **이미지
  탐침은 기본이다**(2026-09-22 S2 round 3): 측정 이미지가 이 호스트에 있으면 **시작하지 않는** 컨테이너의 create(`--pull never --network none` · 규칙 소유 = artifacts 실행기) · cp · rm
  만으로 이미지 안 패치 사본 · 적용 표지 · 원장을 본다(run · start ✗ · 정책 자체검사 계약도 이 모양이다). `--docker-read-only` 가 끈다 —
  그러면 docker 호출은 inspect · history 뿐이고 이미지 표지로만 가를 수 있는 패치는 1.1 에 `unobserved` 로 남는다(태그2 셀: 9개 중 7개).
  `--docker-probe` 는 옛 이름이다(받되 아무것도 바꾸지 않는다).
- **저작**은 발행 Agent 의 일이다. LINEAGE 읽기 목록을 읽고 챕터마다 질문에 답한다: 02 에 벽마다 산문 + `kind: wall` 블록(≥ 1 필수) ·
  기각 · 오진 · 열린 물음 · 01 §1.3 의 서빙 노브마다 `kind: value-status` 블록(전송·신원 노브 `model`·`host`·`port`·`served-model-name` 제외) ·
  원인·해소는 출처의 **원문 발췌**로. 발췌와 서명은 `excerpt` 출력에서 글자 그대로 옮긴다 — 린터가 출처와 대조한다(계약 §8).
  `excerpt` 출력은 **정규화된** 원문이다(`\n` 으로만 줄을 가른다 · tqdm `\r` 진행 조각은 마지막만 · ANSI 색 제거 · 노드 호스트/IP ·
  NIC 장치 이름 · 운영자 경로는 자리표시) — 줄 번호는 grep -n 과 같다(`--numbered` 가 그 번호를 `L<n>` 으로 붙여 보여 준다 — 발췌
  본문에는 번호를 옮기지 않는다). 로그 · .sh · Dockerfile · JSON 같은 비-마크다운 출처의 발췌 머리는 `§L<a>-<b>`(그 줄 번호)이고 마크다운
  출처는 제목 줄의 앞부분이다(계약 §8.3). 측정 도구(sweep_bench · run_bench · lite_bench · 드라이버)의 **측정 시점 원문**은 publish 가
  draft `inputs/sources/<도구>@<rev12>` 에 스냅샷하고 LINEAGE 후보로 올린다 — 발췌 출처 토큰은 `<도구>@<rev12>` 이고 머리는 언제나
  `§L<a>-<b>` 다(03 §3.4 사실 블록이 목록 · `git show` 좌표를 싣는다). bench JSON 에 없는 인자(temperature · ignore-eos · range-ratio)는
  거기서 인용한다. 사실 블록(`<!-- FACT -->`) · 03 부하 곡선 · 00 배너는
  손대지 않는다. 템플릿 최상단의 **저작 자기점검**(`<!-- SELFCHECK -->` · 아래 7부류)을 사실 검증에 넘기기 전에 스스로 훑는다(봉인이 지운다).
  오진 블록의 `현재지위` 는 **원래 주장**(서명에 옮긴 당시 주장)의 지위다 — 반증 = 원래 주장이 거짓 · 확증 = 참 · 가설(강등) = 확인되지
  않아 가설로 내림 · 미결(정정된 이해는 `해소` 칸 · 정본 `template.CURRENT_STATUS_MEANINGS`).
- **발행 전 독립 사실 검증**(필수 · 2026-09-22 S2 round 2 — 린트 0 · 발췌 무결성 통과인 1차 재생 페이로드가 독립 검증에서 사실 오류 25건을
  냈다: 인용이 원문과 같은가는 린터가 보지만 **주장이 원문과 맞는가**는 보지 못한다). 저작이 lint 0 에 이르면 **저작자가 아닌 Agent** 가
  페이로드(00~03 산문 · hint-event)의 사실 주장을 전부 뽑아 인용 출처(계보 문서 · 원장 · 로그 · 이미지 · 인증서)와 대조하고, 아래
  7부류를 **전부** 점검한 결과를 `<draft>/inputs/factcheck.json` 으로 남긴다. 저작자는 지적마다 페이로드를 고치고(또는 주장을 빼고) 그
  항목을 `status: fixed` + `resolution` 으로 닫은 뒤 lint 를 다시 0 으로 만든다.

  | # | 부류(`template.FACTCHECK_CLASSES`) | 검증자가 묻는 것 |
  |---|---|---|
  | 1 | 수치의 귀속 | 이 수치는 어느 셀 · 어느 실행(시도) · 어느 측정의 것인가 — 자매 셀 · 옛 측정 · 다른 캠페인 값이 섞였나 |
  | 2 | 선언 대 측정 | 예산 산식 · 설정 · 원장 선언값을 측정값처럼 적었나(엔진 로그 실측과 나란히 · 어느 쪽인지) |
  | 3 | 가설을 처방으로 | 강등된 기전 · 설계 단계 추론 · 조건이 다른 검증(예: MTP off)을 확정 처방 · 이 셀의 관측으로 적었나 |
  | 4 | 문서 순서 · 정체 | "첫째/둘째 plan" · "이 셀의 attestation" 같은 순서 · 소속 주장이 계보 날짜 · 파일 내용(config · mtime)과 맞나 |
  | 5 | 출처 없는 인과 | "~때문에" 에 출처가 있나(없으면 추론이라고 밝혔나) |
  | 6 | '유일한 차이' 주장 | 셀 간 차이를 하나로 말했다면 인증서 소프트 지문 키 전부와 부하 조건을 대조했나 |
  | 7 | 기록된 것을 미관측이라 함 | "미관측 · 미기록" 이라 적은 것을 원장(01 §1.4 이벤트 표) · docker history · 이미지 안 파일 · 로그가 기록하고 있나 |

  `factcheck.json`(schema 1) = `{schema_version: 1, tag: <draft 태그>, author: <저작 Agent 역할>, checker: <검증 Agent 역할 — author 와
  달라야 한다>, checked_utc, claims_checked: <점검한 주장 수 ≥ 1>, classes_checked: [1,2,3,4,5,6,7], items: [{id, where: "<파일>:<줄|§절>",
  claim, verdict: wrong|misleading|unsupported, class: 1..7(0 = 기타), truth, source, status: open|fixed|disputed, resolution}]}` — 역할 이름으로
  적는다(사람 이름 ✗ · 00 메타에 실린다). continue 는 린트 0 뒤에 이 보고를 본다: 없음 `HINT_FACTCHECK_ABSENT` · 모양 결함(검증자 = 저작자 ·
  7부류 누락 · 다른 태그 · fixed 인데 resolution 없음 등) `HINT_FACTCHECK_INVALID` · 판정 항목 중 `fixed` 가 아닌 것(open · disputed)
  `HINT_FACTCHECK_OPEN`(`unsupported` 도 막는다 — 근거 없는 단정은 수신자에게 오도와 같다). 막히면 **봉인 · 커밋 · ref 쓰기 0** 이다.
  탈출구는 **사람 결정 하나** — `continue --factcheck-waiver "<사람 발화 전사>"` 는 state.json · 00 메타 · `PAYLOAD.factcheck` 에
  `사람 면제` 로 남는다(수신자가 사실 검증 게이트를 통과하지 않은 발행임을 사유코드대로 안다 — 보고 없음 · 모양 결함 · 열린 지적 ·
  에이전트가 스스로 면제하지 않는다). 면제 전사가 빈칸 · 자리표시 · 여러
  줄이면 부수효과 전에 `HINT_FACTCHECK_WAIVER_INVALID` 로 거부한다(계약 §6.7).
- **continue** 는 ① 승인(부수효과 0 — 없으면 `HINT_APPROVAL_ABSENT` 로 멈추고 어느 ref 도 움직이지 않는다) · push 까지 가는 실행이면
  중앙 권위 선언(§6 · 없으면 `HINT_PUSH_CENTRAL_AUTHORITY_ABSENT` · 부수효과 0) → ② 사실 갱신 · 린트 · **사실 검증 게이트** · PROMPT
  봉인(저작 자기점검 주석도 지운다) · 봉인 뒤 린트 → ③ 게이트 · footer 바인딩 확인 → hint 브랜치 배관 커밋(합성 신원 · 주입 시각) → ④ promotion_target 기록 → ⑤ 봉인 ·
  이 태그 1개 로컬 검증 → ⑥ push(정확한 refspec 1개) · 원격 SHA 대조 → ⑦ 카탈로그 파생 · 캠페인 publish 위상 순이다(계약 §6).
  각 단계는 `state.json` 에 남아 **재실행은 멱등**이고, 커밋 시각은 첫 커밋 때 적은 값을 다시 쓴다.
- **캠페인 밖(발행 기록 재생)**: 입력은 이미 있는 발행 기록(`docs/_evidence/<topic>.json` · `evidence_publisher` 가 만든다 —
  `.claude/rules/docs.md` §publisher 계약)이다. `publish --publication <topic> --replay [--node cluster]` → 저작 →
  `continue --draft <draft> --generated-utc <UTC> --approved-by "<사람 발화 전사>" --approved-utc <UTC>`. 재생 publish 는 draft 밖에 쓰지
  않는다 — draft 밖 쓰기는 continue 가 그 발행 기록의 promotion_target 을 새 태그로 다시 적는 것부터다(계약 §6).
- **멈춰 확인하고 싶으면** `continue --no-push`(봉인 · 로컬 검증까지) → 이어가기는 **continue 재실행**이다.

## 3. 명령 일람

| 명령 | 부수효과 | 게이트 | 요지 |
|---|---|---|---|
| `publish` | draft(캠페인 모드는 셀 발행 기록 · work-manifest 도 · 측정 도구 스냅샷 `inputs/sources/`) · 시작하지 않는 컨테이너 create/rm(탐침 · 기본) | — (사전 확인만) | `--campaign/--cell` 또는 `--publication <topic> --replay` · `--node` · `--generated-utc` · `--lineage-add PATH=REASON`(끊긴 계보 사람 보충) · `--out` · `--remote` · `--no-remote-check`(오프라인 · 로컬 충돌만) · `--docker-read-only`(이미지 탐침 ✗ — inspect · history 만 · 이미지 안 원장 · 패치 사본은 관측 실패로 기재) · `--docker-probe`(옛 이름 · no-op — 탐침은 기본이다: create(`--pull never`)·cp·rm 만 · run/start ✗) |
| `continue` | hint 브랜치 커밋 · 태그 · push · 카탈로그 · 캠페인 위상 | `hint_finalize` · `hint_push` · push 까지 가면 중앙 권위 선언(D8) · 발행 전 독립 사실 검증(`inputs/factcheck.json`) | `--draft` 또는 `--campaign/--cell [--node]` · `--remote`(기본 origin) · `--no-push` · 캠페인 밖 `--approved-by/--approved-utc` · 사람 면제 `--factcheck-waiver "<발화 전사>"` |
| `lint` | 0(사본에서 파생 블록 반영 후 판정) | — | `--json` · rc 1 = 결함 있음 · 텍스트 출력 끝에 사실 검증 보고 상태(게이트 판정 미리 보기) |
| `refresh` | draft 안 00 §0.3 · §0.5 파생 블록만 | — | 저작 중 파생 요약 미리 보기(lint 는 여전히 사본을 refresh 해 판정) · 커밋 뒤 거부(`HINT_DRAFT_ALREADY_COMMITTED`) |
| `excerpt` | 0 | — | 린터가 대조하는 **정규화 · 치환 후 원문** 출력 · `--draft` · `--source <stem\|경로\|artifacts 파일명\|<도구>@<rev12>>` · `--lines a-b`(= 비-마크다운 발췌 머리 `§L<a>-<b>` 의 번호) · `--numbered`(줄마다 `L<n>` — 그 번호를 보여 준다) |
| `name` | 0 | — | 파생 이름 · 세그먼트 · 축별 출처 · 빌드 입력(`--json`) |
| `verify --tag T` | 0 | `hint_verify` | 이 태그 1개 · 로컬 봉인 검증(서사 린트 포함 → 봉인한 draft 필요 · `--draft`) |
| `push --tag T` | `--apply` 때만 원격 쓰기 | `hint_push` · 중앙 권위 선언(D8) | 정확한 태그 1개 · 기본 dry-run(실패 안 삼킴) · 카탈로그 · 위상은 하지 않는다(continue 가 한다) |
| `catalog derive` | `hints/index.json` · `HINTS.md` | 중앙 권위 선언 | `--remote` · `--generated-kst` · `--record-missing`(로컬 오브젝트 없는 원격 태그를 정직한 행으로 · rc 0) · `--dry-run` · `--allow-empty`(원격 hint 태그 0건을 정상으로 선언) |
| `match` | 0 | — | 수신자 근-미스 발견 · gitless(`hints/index.json` 만) · 정규화 슬러그 + `base_model` |
| `--self-test` | 0(격리 임시 저장소) | — | 전 모듈 자체검사 + 파서 + 격리 E2E(라이브 태그 · 브랜치 · 캠페인 비의존) |

전역 `--repo R` 은 **서브명령 앞**에 둔다(2026-09-12: 인자 순서가 틀린 호출이 한 번도 성공한 적 없었다). 게이트 거부는 게이트 JSON 을
stdout 에 그대로 내고 그 종료코드로 끝난다(게이트를 실행하지 못하면 같은 모양의 포장 JSON · exit 2). 그 밖의 실패는 `HINT_*` code 와
처방(remedy)을 stderr 에 낸다 — 판정은 code 로만 한다.

## 4. 무엇이 실리나

```
README.md · 00-hint.md(지도 — 먼저 읽는다) · 01-artifacts.md · 02-narrative.md · 03-benchmark.md ·
PAYLOAD.json · LINEAGE.json · PROVENANCE.json · artifacts/<slot>/…(적용된 것만) · (multi) artifacts/compose/sub_recipe.json
```

- annotated 태그 본문은 brief 한 문단 + "zip 의 `00-hint.md` 부터" 포인터 + 증거 주소 footer 뿐이다 — 지도는 zip 안에 있다(계약 §7.3).
- `artifacts/` 는 **그 셀이 실제로 쓴 것만** 싣는다: 쓰인 Dockerfile 1종과 그것이 COPY 하는 파일(소스빌드도 `requirements.txt` — D-a) ·
  compose · `serve_runner.sh` · env **형상** 템플릿(실물 `.env` ✗) · 적용된 빌드 패치. skip 된 패치 · 쓰이지 않은 Dockerfile · 다른 평면
  파일은 싣지 않는다. 측정 **뒤** 재생성된 env 형상도 싣지 않는다(렌더러가 바뀌면 측정 당시 없던 키 · 다른 값이 실린다 — 01 §1.1 '싣지
  않은 파일' `post-measurement-regenerated`) — 측정 당시 값은 측정한 실행의 엔진 로그 env echo(01 §1.4 '측정 실행의 env 관측')가 사실로
  대신한다. compose 슬롯 추적 파일도 Dockerfile 처럼 측정 당시 개정을 싣는다(01 §1.1 '실린 리비전'). 적용 판정 1순위는 이미지 안 빌드 원장, 원장 없는 옛 이미지는 `unobservable` + 라벨된 재구성(계약 §3 A).
- 결손은 차단이 아니라 **기재**다(00 · `PAYLOAD.missing[]` · 카탈로그 결손 열). 부재로 막는 자리는 닫힌 목록이다 — 관측 · A 층 · 여정 ·
  셀 서사 증거(plan·devlog·testlog) · 계측 바인딩(인증서 또는 bench_report) · 이름·노드 축의 증거 · 승인 기록 · `pii_terms.txt`. 그 밖의
  차단은 양성 검출이다(계약 §3.1 표 · §5 표).

## 5. 이름 — 입력하지 않는다

`hint/<vllm>/<model>/<arch>/<recipe>` — 예 `hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvauto-plemmap-spec3-eager`.
`<vllm>` = 빌드 입력(릴리스 태그 또는 `<직전 릴리스>-g<sha12>`) · `<arch>` = `<hw>-<G>g<N>n-<main|sub|cluster>-<target>` · `<recipe>` =
고정 6축 전부. 어휘는 `hints/vocab.json`(사람 편집 tripwire). 어휘 밖 = `HINT_VOCAB_UNKNOWN`(추가할 키를 알려 준다) · 파생 불가 =
`HINT_AXIS_UNDERIVABLE`(없는 증거를 알려 준다) · 충돌 = `HINT_NAME_COLLISION`(태그는 불변 — 개정판은 축이 모자란 것이다). 정의 전체는
계약 §4.

## 6. 권한 — 누가 어느 원격의 카탈로그를 소유하는가

카탈로그 갱신(`catalog derive` · continue 의 마지막 파생)은 `hints/.central_authority`(**비추적**) 가 있는 체크아웃만 한다. 마커는
자격증명이 아니라 **역할 선언**이다 — 배포본에 실려 복사되면 안 되므로 각 저장소가 자기 원격에 대해 스스로 선언한다:

```bash
printf '%s\n' '이 체크아웃이 자기 원격의 hint 색인·배포 권위다.' > hints/.central_authority
```

선언은 권한이자 책임이다(그 원격의 `index.json`·`HINTS.md` 정합을 떠안는다). 상류에 기여하는 입장이면 선언하지 말고 `continue --no-push`
로 로컬 태그까지만 만들어 상류에 전달한다 — **선언이 없는 체크아웃은 push 하지 않는다**(옛 D8 · 2026-08-20: 봉인은 분산, 색인·배포는
중앙). 도구가 이것을 집행한다 — 선언이 없으면 push 까지 가는 `continue` 와 단독 `push` 는 **커밋·봉인 전에**
`HINT_PUSH_CENTRAL_AUTHORITY_ABSENT` 로 멈춘다(2026-09-22 통합에서 옛 hint_tag push 게이트 복원 · 부수효과 0). ⚠ 어느 쪽도 아니면 우회하지 말고 어느 쪽인지부터 정하라 — 맨 `git tag -a` + `git push` 로 만든 태그는 증거
footer 도 페이로드 형식도 없고, 카탈로그는 원격 태그에서 파생되므로 그런 태그도 목록에 들어간다(정문으로 가라).

## 7. 경계

- **무인 자동 태깅 ✗** — 승인 기록 없이는 어느 ref 도 움직이지 않는다(§1).
- **main-only** — 서브는 발행하지 않는다. 서브 산출물은 문서기반 회수 뒤 메인이 재저작한다(`docs.md` 상향 회수 규약 · 계약 §6.6).
- **태그는 불변 · 리콜 금지** — 발행 후 이름 · 본문을 고치지 않고, 원격의 과거 태그는 판정 · 교정 · 삭제하지 않는다. 개정은 새 태그로만.
- **게이트 = 이번 발행분만** — verify · push 는 이 태그 1개만 본다. 과거 · 타 PC 태그의 결함은 신규 발행을 막지 않는다(계약 §6.3).
- **push = 정확한 태그 1개**(`refs/tags/<그 태그>` · `--tags` ✗ · glob ✗ · `--no-follow-tags`). **hint 브랜치는 발행이 밀지 않는다** — 형식
  전환 커밋의 1회 선행 push 는 plan S7(G2)의 예외이고 평시 브랜치 push 는 사용자 소관이다. ssh · 로컬 원격은 토큰 불요, https 는
  `GITHUB_TOKEN`(환경 또는 `envs/.env`)(계약 §6.2).
- **카탈로그는 손으로 쓰지 않는다** — 진실원천은 `git ls-remote`. 조회 실패 = 캐시로 대체하지 않고 중단. `HINTS.md` 는 머리말까지 생성물.
- **손 JSON 0** — identity · runtime · pii · promotion_target 은 도구가 쓴다. `state.json` 도 손으로 고치지 않는다. 예외는 하나 —
  `inputs/factcheck.json` 은 **검증 Agent** 가 쓰는 보고다(저작자가 대신 쓰지 않는다 · 도구는 모양과 열린 항목만 판정한다).
- **사실 검증 면제는 사람 결정만** — `--factcheck-waiver` 에는 사람 발화 전사만 넣는다(승인 전사와 같은 규율). 면제는 배포 본문에 남는다.
- **hint 브랜치를 체크아웃하지 않는다** — 배관만 쓴다(브랜치 전환이 산출물을 파괴한 실측 이력). hint 브랜치는 산출물이라 `sync_branches` 대상이
  아니다. 커밋 author/committer 와 tagger 는 **합성 신원**이고 실명 override 경로는 없다.
- **시각은 주입만**(`--generated-utc` · `--generated-kst`) — 벽시계 ✗.

## 8. 알려진 한계

- native 평면 셀은 평면 신호 producer 가 없어 `HINT_PLANE_UNDERIVABLE` 로 막힌다(D-d · fail-closed 유지).
- 서브에서만 잰 셀은 메인이 관측을 볼 수 없어 `HINT_QUALIFICATION_UNOBSERVED` 로 막힌다(D-e · 열린 설계) — cluster 셀은 메인이 관측한다.
- 두 번째 마디가 10 인 4마디 vLLM 릴리스 이름은 사설 IPv4 패턴에 걸려 `HINT_TAG_NAME_PII` 로 먼저 막힌다(스캐너 체계 변경은 사람 결정).
- 단독 verify · push 는 봉인한 draft 가 있어야 한다(`HINT_LINT_DRAFT_ABSENT`) · 끊긴 publish 는 **같은** `--generated-utc` 로
  다시 한다(`HINT_PUBLICATION_TIME_BOUND`) · 재생 draft 의 continue 는 옛 발행 기록의 promotion_target 을 새 태그로 다시 적는다.
- 전체 목록과 사유는 계약 §10.

## 9. 이력

- 2026-08-20 `upstream-version-watch` 에서 스킬로 승격(`plan_26082009`) · 2026-09-01 태그가 hint 브랜치 페이로드 커밋을 가리키게 전환 ·
  감사 `audit_26090106`(21건)·`audit_26090118`·`audit_26090708`.
- 2026-09-22 S2 round 2(`plan_26092119`): 오프라인 재생 채점(여정 91.6 · 체크리스트 86.1 · 오도 바이트 12.7KB · 사실 오류 25건)의
  처방 — 발행 전 독립 사실 검증 게이트(7부류) · 템플릿 저작 자기점검 · `refresh` · 발췌 출처 정규화(`\n` 줄 · `\r` · ANSI) · 비-마크다운
  `§L<a>-<b>` · NIC 장치 이름 치환 · brief 수치 대조 · declared-requirement 근거(W id 또는 artifacts 헤더) · 3.5 비교 규칙 = 인증서 소프트 지문 ·
  사실 블록(이벤트 타임라인 · 현행 full 정의 · HF revision · 드라이버 · 결과 출처 범례 · 선택 리비전 · 측정 뒤 재생성 파일).
  같은 날 적대 리뷰 정정: 최상위 interconnect(두 노드 공통)의 NIC 이름 = `<nic:cluster>`(1판 `<nic:main>` 은 서브 로그 줄을 오귀속) ·
  declared-requirement 의 artifacts 근거에서 트리플렛(값을 선언한 파일 자신) 제외 · brief 수치 허용 집합에서 파생 블록(저작자 hint-event) 제외 ·
  기동 시도 묶음은 원장 kind 정확 일치(`budget_declare_rejected` 는 시도가 아니다 · 사살 · 트립은 닫힘과 따로).
  같은 날 통합 정정: `--docker-probe` 도움말 = 이미지 원장도 create+cp 로 읽는다(옛 "원장 cat 관측 실패" 는 경로가 바뀐 뒤 거짓) ·
  빈 이벤트 타임라인 사실 블록 = "원장 부재와 행 0 을 구분하지 않는다"(evidence 는 둘 다 `[]` — 옛 "evidence 가 가른다" 는 오도) ·
  사람 면제 문구 = 사유코드대로(보고 없음 · 모양 결함 · 열린 지적) · `HINT_FACTCHECK_WAIVER_INVALID` 명기.
- 2026-09-22 S2 round 3(`plan_26092119` · 2차 채점: 여정 가중 92.5/91.1 · 체크리스트 91.7/94.4 · 오도 바이트 1,112B · 사후 사실 오류 7 ·
  패치 판정 7/9 미관측 — publish 가 이미지를 탐침하지 않았다): **이미지 탐침 = publish 기본**(시작하지 않는 컨테이너 create · cp · rm ·
  `--docker-read-only` 가 끈다 · `--docker-probe` = no-op 옛 이름 · 정책 자체검사 docker 계약도 같은 모양으로) · `excerpt --numbered`(§L 번호) ·
  발췌 출처에 측정 도구 스냅샷(`<도구>@<rev12>` = draft `inputs/sources/` · LINEAGE 등재분만) · 새 사실 블록(01 §1.4 측정 env 관측 · 01 §1.5
  노드별 접기 · 03 §3.4 측정 도구 원문 · 00 §0.4 vLLM 버전 문자열마다 생산자 · attestation 범위 줄 · 01 §1.1 복수 리비전(compose 포함) ·
  싣지 않은 파일 · 패치별 판정 근거 · 빌드 층 CreatedAt 창) · PROMPT(3.5 acceptance length = 출력(비로 비교) · spec 켜짐/꺼짐 = 조건 ·
  이전 측정 레벨 전부 나란히 · 0.2/2.5 버티는 기전 대 실패 기전 + 지위 · 2.4 교정 묶음 항목 id · 현재지위 = 원래 주장의 지위).
  같은 날 적대 리뷰 정정: 스냅샷은 LINEAGE origin(`git show <rev>:<경로>`)과 바이트 동일할 때만 발췌 출처(`HINT_EXCERPT_SNAPSHOT_DRIFT` ·
  `_UNVERIFIABLE` — 스냅샷 파일을 고친 발췌가 통과하던 구멍) · 01 §1.5 노드 접기 = Ray 가 접은 줄(`[repeated Nx across cluster]`)은 '한쪽만
  관측' 으로 다른 쪽 부재를 단언하지 않는다(태그2 라이브 6키 오도) · 같음 = 값 집합 · 0.2 vLLM 행이 0.4 의 생산자를 가리킨다(같은 값일 때만) ·
  도구 스냅샷 다음 변경 없음 = `관측 없음`(변경 없음 단언 ✗) · publish 저작 안내 = 스냅샷 토큰 · `--numbered` · 탐침 규칙은 artifacts
  실행기 하나(hint.py 가 한 벌 더 적던 규칙은 `--network none` 을 빠뜨렸다).
  같은 날 통합 정정: 측정 기록 행(`measurement`)은 기동 시도가 아니다(01 §1.4 · 02 §2.2 시도 표에 노드 '미관측' 가짜 시도가 섰다) ·
  꼬리 캡처 로그(생산자 `log_window` · 태그2 = 전부 tail-800)의 한쪽 관측은 다른 쪽 부재를 단언하지 않는다(01 §1.4 로그 창 주 · 01 §1.5
  판정) · 도구 스냅샷 `다음 변경` = 생산자 `next_rev_basis` 그대로('변경 없음' 과 '후손 아님' 을 가른다) · 배포 문구의 내부 좌표 제거
  (`facts.measurement_env_observed` → 01 §1.4 표 · PAYLOAD.json · 결손 뜻의 '코드맵' 좌표) · 미관측 패치의 처방 = `--docker-read-only`
  를 빼고 다시(no-op 옛 플래그를 처방으로 적지 않는다).
- 2026-09-22 재구성(`plan_26092119` S1–S5): 옛 CLI(`hint_tag`·`hint_collect`·`hint_branch`·`hint_catalog`·`bootstrap_families`)와
  family 색인(`families.json`)·옛 본문 템플릿을 **shim 없이** 제거하고 단일 진입 `hint.py` + `hintlib/` 로 옮겼다. 옛 린터 L1–L5 는 공허 검사가
  섞여 있어 폐기하고 PROMPT 템플릿 린터로 대체했다 — **템플릿만으로는 아무것도 보장되지 않는다, 집행되는 검사만 지켜진다**(2026-08-20
  실측: 49/49 태그 헤딩 0개).
