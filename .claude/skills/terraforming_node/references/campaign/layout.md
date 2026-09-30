# 캠페인 — 배치와 수명

> terraforming_node 스킬 reference — workflow.md §배치와 수명 + docs.md §캠페인 워크스페이스 경로표. 캠페인 절차의 **정본**은 이 폴더다(workflow.md 는 불변식 · CLAUDE.md 는 "왜").
> 이관 전 원문: `git show dcb713a:.claude/rules/workflow.md` §캠페인 아티팩트 체인 (plan_26093022 Step 4 — 본문 바이트는 그대로 옮겼다). 불변식은 workflow.md 에 남는다.


> 왜: 여러 버전×모델을 순차로 도는 캠페인에서 단계 간 정보를 **대화 기억이 날랐다**. 세션이 끊기거나
> 문맥이 압축되면 그 정보가 사라졌고, 각 스킬은 자기 기본값(루트 `config.yaml`·루트 `tasks/`)으로
> 되돌아가 산출물을 관리범위 밖에 흘렸다(2026-09-06 실측: 루트 추적 누출 21개 · hint 태그 2/3 발행).
> 처방은 규율이 아니라 **거처**다 — 나를 것을 파일로 만들고, 그 파일의 자리를 선언에서 파생시킨다.

### 배치와 수명

| 자리 | git | 수명 | 소유 |
|---|---|---|---|
| `campaigns/README.md` · `campaigns/_template/**` | **추적** | 영구(뼈대) | 사용자가 관리하는 유일한 부분 |
| `campaigns/<camp-id>/**` | **비추적** | 캠페인 1회(휘발) | 에이전트가 저작 |
| `campaigns/_bootstrap/**` | 비추적 | 활성 캠페인이 없을 때의 예약 인스턴스 | 온보딩·카나리 릴레이 |

- **뼈대만 추적**하는 이유: 인스턴스는 운영자 절대경로·세션 id·측정 원시값을 담아 배포 평면에 실릴 수
  없고, 매 캠페인 재생성되므로 이력으로 남길 가치가 git 이 드는 비용을 넘지 않는다. 사용자는 빈칸의
  **모양**만 관리하고 값은 관리하지 않는다.
- **무결성 해시를 두지 않는다** — 뼈대는 추적물이라 git 이 이미 바이트를 든다(`policy:GIT_SINGLE_AUTHORITY`
  2문항 Q1 = 예 → 중복층). 인스턴스는 휘발이라 대조할 두 번째 자리가 애초에 성립하지 않는다.

### 경로 전수 (docs.md §캠페인 워크스페이스에서 이관)

| 경로 | 내용 | git | 수명 |
|---|---|---|---|
| `campaigns/README.md` | Agent 읽기 순서·채우기 규칙 | 추적 | 영구 |
| `campaigns/_template/**` | 뼈대(스키마·선언·셀·phase·릴레이·스윕 틀) | 추적 | 영구 |
| `campaigns/<camp-id>/campaign.yaml` | 캠페인 선언(matrix·순서·예산·통제변인·hint 대상) | 비추적 | 캠페인 1회 |
| `campaigns/<camp-id>/cells/<cell>/` | 셀 입력(`config.yaml`·`lockset.json`)과 상태 | 비추적 | 동상 |
| `campaigns/<camp-id>/phases/<node>/` | phase 상태+proof | 비추적 | 동상 |
| `campaigns/<camp-id>/relay/` | A2A 릴레이 원장(옛 `tasks/`) | 비추적 | 동상 |
| `campaigns/<camp-id>/sweeps/` | 스윕 상태·정지판정 | 비추적 | 동상 |
| `campaigns/<camp-id>/evidence_pointers.json` | docs 평면 증거 포인터(purge 선행조건 · publish 위상에서 `frozen_utc` 로 동결) | 비추적 | 동상 |
| `campaigns/<camp-id>/journey.jsonl` | **여정** — 이탈·반증·축 이동 사유와 다음 의도(append-only) | 비추적 | 동상 |
| `campaigns/ACTIVE` | 살아 있는 인스턴스 **하나**의 이름(한 줄). 부재·무효 = `_bootstrap`(루트 ✗) | 비추적 | 캠페인 1회 |
| `campaigns/_bootstrap/` | 캠페인 밖 릴레이 **대기실**(온보딩·카나리). purge 게이트 대상 ✗ · 새 init 때 함께 비운다 | 비추적 | 상시(내용은 휘발) |

### 파생 선언 (docs.md §보관·전파 matrix 에서 이관)

| 경로 | Git 상태 | main/sub 전파 | owner/gate |
|---|---|---|---|
| **파생 선언**(`campaign_init --emit-slice <node>` 산출) | ignored(휘발) | **메인→서브 단방향**(독립 campaign executor만 relay 지시서 JSON→stdin으로 전달 · 2026-09-18) | 배정 SSOT 는 메인 `assignments`; sub는 `--init --from-slice -`로 stdin 선언을 소비한다. `--from-slice <PATH>`는 호환 입력이며, 독립 instance를 열지 않는 sub mode에는 보내지 않는다. |

- 바이트를 쓰는 문은 `campaign_init.py` 하나다(`writers.md`). 읽는 눈은 `--resume-brief` 이며 `campaigns/README.md` 읽기 순서 **0번**이다.
