# request_<YYMMDDHH>_서브_hint_push_자격_배치

> **수행지시서 템플릿**(추적 · PII-free) — terraforming_node `references/install/sub_agent_env.md` §2.5.1 hint 발행 장착이 쓴다.
> 메인 Agent 는 서브 testlog 의 `check_sub_hint_equip.py` 보고에서 `next_actions[].actor = "human"` 을 읽었을 때만
> 이 파일을 복제해 `docs/request/request_<YYMMDDHH>[_MM_SS]_서브_hint_push_자격_배치.md` 로 발행한다(이름은
> `wiki-desk/src/doc_naming.ts` 로 실제 KST 시각에서 파생 · `<…>` 자리는 발행 시 채운다 · 이 인용 블록은 지운다).
> 왜 사람인가: push 토큰과 그 노드의 PII 리터럴은 **비밀**이라 배달 평면(render_sub_env · sync_to_sub)으로 보내지 않는다.
> 에이전트는 값을 만들지도 옮기지도 않고, 둔 뒤에 **존재만** 확인한다.

## ① 전제·준비물

- 대상: single 서브 노드 1대(manifest `nodes[sub]` · sub_mode `a2a-agent`). 그 노드에 셸로 로그인할 수 있는 사람.
- 서브 워크스페이스 경로: `<SUB_WORKSPACE>` — 얻는 법: 메인 `output/single/manifest.yaml` 의 `nodes[sub].work_dir`.
- 토큰: 프로젝트 원격에 **태그 push 권한만** 가진 fine-grained PAT(contents: write 범위 · 만료일 지정 권장).
  원격이 ssh(`git@…`) 또는 로컬 경로면 토큰은 필요 없다 — 그 경우 ③-A 를 건너뛴다.
- 이번 보고가 요구한 항목(서브 testlog `next_actions[].what` 그대로): `<WHAT_LIST>`

## ② 단계별 명령 (서브 노드에서 · 사람이 직접)

```bash
cd <SUB_WORKSPACE>
# A) push 토큰 — envs/.env 는 비추적·비배달 평면이다(서브 .gitignore 가 덮는다). 값은 화면·로그에 남기지 않는다.
umask 077
touch envs/.env
read -rsp 'PAT: ' T; echo                     # 입력이 화면에 찍히지 않는다
grep -q '^GITHUB_TOKEN=' envs/.env || printf 'GITHUB_TOKEN=%s\n' "$T" >> envs/.env
unset T
chmod 600 envs/.env
# B) 그 노드의 PII 리터럴 목록 — 한 줄에 리터럴 하나 · `#` 주석. 정말 없으면 빈 파일이 "없음" 선언이다.
#    (.claude/ 는 메인 소유라 서브 에이전트는 쓸 수 없다 — 그래서 사람이 둔다.)
#    이미 파일이 있으면 그 노드의 목록인지 먼저 열어 본다 — 옛 배달이 남긴 메인 목록 사본일 수 있다(2026-09-29 실측).
test -f .claude/pii_terms.txt || : > .claude/pii_terms.txt
chmod 600 .claude/pii_terms.txt
```

## ③ 성공 판정

```bash
cd <SUB_WORKSPACE>
python3 .claude/skills/terraforming_node/scripts/check_sub_hint_equip.py | python3 -c \
  'import json,sys; c=json.load(sys.stdin)["checks"]; print(c["token"]["status"], c["pii_terms"]["status"])'
```

- 기대: `present present`(ssh·로컬 원격이면 `not_required present`). 토큰 **값**은 어디에도 출력되지 않아야 한다.
- `git -C <SUB_WORKSPACE> status --short` 에 `envs/.env` · `.claude/pii_terms.txt` 가 **보이지 않아야** 한다(추적되면 안 된다).

## ④ 실패 시 분기

| 증상 | 조치 |
|---|---|
| `token: absent` | `envs/.env` 에 `GITHUB_TOKEN=` 줄이 있는지(앞 공백·따옴표만 있는 빈 값은 없음으로 본다) |
| `status --short` 에 파일이 보인다 | 서브 `.gitignore` 가 낡았다 — 직접 고치지 말고 메인에 보고한다(메인 재배달로 고친다) |
| push 단계에서 403 | 토큰 권한(contents: write)·대상 저장소를 확인한다. 에이전트에게 토큰을 붙여 넣지 않는다 |

## ⑤ 회수물 (에이전트에게 돌려줄 것)

- `check_sub_hint_equip.py` 출력 JSON 전체(비밀 미포함) — 메인이 다음 A2A 지시로 서브에게 다시 돌리게 해도 된다.
- 토큰 만료일(날짜만). 토큰 값 · PII 리터럴 내용은 **회수물이 아니다**.

## ⑥ 예상 소요·비용

- 사람 5~10분(PAT 발급 포함). 비용 없음.
