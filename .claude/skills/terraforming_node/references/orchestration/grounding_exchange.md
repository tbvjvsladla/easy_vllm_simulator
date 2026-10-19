# 오케스트레이션 — 서고 접근자 (그라운딩)

> terraforming_node 스킬 reference로, SKILL.md §2.7.8의 본문이다. SKILL.md(라우터)는 § 번호·제목·포인터만 둔다.
> **2026-10-19 개정(plan_26101912)**: A2A 3메시지 서고 교환(`library_request` → relay → export → attestation ·
> `library_exchange.py`·`library_relay.py`)은 **은퇴**했다. 병존 폴백도 두지 않는다. 옛 본문은 `git show 57723fc:<이 경로>`에서 볼 수 있다.

## 목차

- 무엇이 바뀌었나 — 같은 CLI, 다른 경로
- 접근자 — manifest `library` 블록(메인·서브 동일 스키마)
- 게이트웨이 — 서브가 얻는 것은 셸이 아니라 사서 명령 하나
- 발급·회수 — `scripts/library_access.py` (HITL)
- 그라운딩 게이트 — 메인·서브 대칭
- 동시성 — 정본은 `.claude/skills/wiki-desk/references/concurrency.md`

### 2.7.8 서고 접근자 — 서브 wiki-desk ↔ 메인 서고 (2026-10-19 · `plan_26101912`)

> 헌법 **불변식 B**의 "어떻게"다. 헌법은 *인용 없는 결정은 성립하지 않으며, 서고는 하나이고 접근점은 N개*라고 말한다.
> 이 절은 *그 접근점이 무엇이고 누가 발급하는가*를 정한다.

#### 무엇이 바뀌었나 — 같은 CLI, 다른 경로

| | 옛(은퇴) | 지금 |
|---|---|---|
| 서브가 근거를 얻는 법 | 리포트 `library_request[]` → 메인 relay가 사서에 질의 → 다음 턴 본문에 반출 | 서브 wiki-desk가 `query`/`read`/`tree`를 **직접** 실행한다(메인 게이트웨이로 전달) |
| 서브 문서의 입고 | `fetch_sub_docs.sh` 미러를 메인이 warm-start | 서브 `sync --apply` = 반입(rsync) + 원격 입고. 발행 도구가 자동으로 부른다 |
| 판정 | `library_exchange.py` 3질문 게이트(attestation) | **서빙 진입 전 그라운딩 기록 게이트**(`campaign_init --ground/--grounding-check`)를 메인·서브에 대칭으로 둔다 |
| 병목 | 질의 1회 = relay 턴 1회 | 질의 1회 = ssh 1회 |

메인 wiki-desk와 서고의 관계는 **경로만 다르고 같다**. 패키지 바이트도 같다(무결성 권위 = git · 렌더가 추적물만 복제한다).

#### 접근자 — manifest `library` 블록

```yaml
# 메인 output/<t>/manifest.yaml
library:
  transport: local
  wiki_dir: __llm-wiki
  repo_root: "<메인 work_dir>"
  access_points:                 # library_access.py 가 쓴다(서브 접근점 N개)
    sub:
      identity_file: "<서브 HOME>/.ssh/wiki_desk_ed25519"
      known_hosts: "<서브 HOME>/.ssh/wiki_desk_known_hosts"

# 서브 발급본 output/<t>/sub_manifest.yaml → 서브 output/<t>/manifest.yaml
library:
  transport: ssh
  host: "<메인 host>"
  user: "<메인 ssh_user>"
  repo_root: "<메인 work_dir>"
  wiki_dir: __llm-wiki
  identity_file: "<서브 HOME>/.ssh/wiki_desk_ed25519"
  known_hosts: "<서브 HOME>/.ssh/wiki_desk_known_hosts"
  node_id: sub
```

- wiki-desk `src/accessor.ts`가 이 블록을 읽는다. 특화헌법 헤더 `**topology: X**` 로 `output/X/manifest.yaml`을 고르고, 브랜치에서 추론하지 않는다.
- 블록이 없으면 local로 둔다(출처 `default:no-library-block`). 블록이 있는데 형식이 틀리면 거부한다(침묵 폴백 ✗).

#### 게이트웨이 — 서브가 얻는 것은 셸이 아니라 사서 명령 하나

메인 `~/.ssh/authorized_keys`에 서브 전용 키 1줄이 들어간다.

```
command="<node> <repo>/.claude/skills/wiki-desk/src/gateway.ts --node <node_id>",restrict ssh-ed25519 <키> wiki-desk-gateway:<node_id>
```

- 허용은 두 모양뿐이다. ① `wiki-desk-v1 <base64url(JSON argv)>` — 닫힌 명령 목록(query·read·tree·check·status·contract·sync)과 플래그 목록만 받는다. ② `rsync --server` 수신 — `rrsync -wo <자기 미러>`가 받는다. 그 밖(셸·임의 명령·`--sender` 반출용 rsync)은 exit 126으로 거부한다.
- **node_id는 이 줄이 정한다.** 게이트웨이는 쓰기(`sync`)에 `--as <node_id>`를 못박는다. 서브는 자기 미러 `sync_staging/library_mirror/<node_id>/**`의 카드만 쓸 수 있다(C12 키별 단일 소유).
- argv는 셸을 거치지 않는다(JSON → base64url 토큰 하나). 그래서 인용·주입 문제가 생길 자리가 없다.
- `restrict`는 pty·포트포워딩·에이전트포워딩·X11을 모두 끈다.

#### 발급·회수 — `scripts/library_access.py`

```bash
python3 .claude/skills/terraforming_node/scripts/library_access.py --topology <t> --node <id> --plan
python3 .claude/skills/terraforming_node/scripts/library_access.py --topology <t> --node <id> --apply --approved-by "<사람 발화 전사>"
python3 .claude/skills/terraforming_node/scripts/library_access.py --topology <t> --node <id> --revoke --approved-by "<전사>"
```

1. **사전검사**를 한다. 메인에는 node≥24·flock·rsync·rrsync·서고 계약이 있어야 하고, 서브에는 node≥24·rsync가 있어야 한다. 하나라도 없으면 exit 3으로 **차단 사유를 보고**한다. 자동 설치는 하지 않는다.
2. **서브 전용 키**는 서브에서 만든다. 개인키는 서브를 떠나지 않는다.
3. **메인 호스트 키**를 서브 전용 known_hosts에 넣는다. TOFU(첫 접속 신뢰)를 쓰지 않는다.
4. 메인 authorized_keys에 강제명령 줄을 넣는다. 마커로 멱등 교체한다. **메인 authorized_keys 변경은 매번 HITL이다**(`--approved-by` 없으면 차단).
5. manifest `library` 블록을 메인과 `sub_manifest.yaml` 양쪽에 쓴다. 이후 `render_sub_env.py --sub-manifest` → `sync_to_sub.sh --provision`으로 배달한다(wiki-desk 패키지는 tool plane으로 함께 간다).

- 회수(`--revoke`)는 노드를 해제하거나 재설치할 때만 한다. 주기적인 키 회전은 없다(D11).
- 이것은 노드 제어 불변식 1(무단 스캔 금지)과 충돌하지 않는다. 서브가 메인에 *쓰는* 것은 자기 문서(반입)와 자기 카드뿐이고, 상향 회수는 여전히 문서 기반이다. 메인이 서브를 읽는 통로는 이 절과 무관하다.

#### 그라운딩 게이트 — 메인·서브 대칭

- `campaign_init.py --ground --utc <t>` 는 통제변인에서 파생한 질의로 사서를 부른다. 그리고 `campaigns/<id>/grounding/<utc>.json`에 request·export·attestation 모양으로 기록한다. 서브는 같은 함수가 접근자(ssh)로 메인 서고에 묻는다.
- 판정은 `campaign_init.py --grounding-check`(exit 4)가 한다. 이 판정은 `single_serve_up.sh`·`run_trial.py` 진입 백스톱에서 돈다. **못 찾음(unresolved)은 통과**한다(정직한 공백은 기재 후 진행). **거절(refused = 게이트웨이 exit 126)만 차단**한다. 서고에 닿지 못한 경우는 사유를 단 unresolved로 기재한다.
- 누락은 기계가 잡는다(기록 부재 = 서빙 진입 차단). 거짓은 사람이 리뷰한다. 결정이 인용한 `path:시작-끝`이 실제로 그 결정을 뒷받침하는지는 사람이 읽는다.

#### 동시성

N개 접근점이 함께 쓰는 규정의 정본은 `.claude/skills/wiki-desk/references/concurrency.md`다(policy C6). 핵심은 다음과 같다. flock 배타·공유 잠금, 쓰기 전 저널(kill 뒤 다음 보유자가 롤백), 멱등 원장(node_id + request_id + payload sha256), 키별 단일 소유. fencing token은 도입하지 않았고, 그 근거도 그 문서에 있다.

- 두 토폴로지에 공통이다(§2.7.0 분기표). 서브가 Ray 워커든 A2A 에이전트든 서고 접근의 모양은 같다.
