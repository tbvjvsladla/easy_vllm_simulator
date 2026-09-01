# terraforming_node — 코드레벨 감사

> 감사 시각: 2026-09-01 09:36 KST (KST 절대시각 `26090109`)
> 대상 브랜치: `single-node` · 대상 커밋: `d9fbdfb`
> 감사 범위: `.claude/skills/terraforming_node/` 전량 — 43파일 · 14,280행
> (`SKILL.md` 694행 · `scripts/` 6종 3,974행 · `scripts/node_blackbox/` 17종 7,929행 ·
> `scripts/host_safety/` 4종 420행 · `sub_node/` 7종 · `templates/document_skeletons/` 5종)
> 판정 기준 정본: `CLAUDE.md` · `.claude/rules/workflow.md` · `.claude/rules/docs.md` · 대상 `SKILL.md`
> 감사 방식: PR Review Toolkit 전문 에이전트 **7종 병렬** 정적감사 + 감사자 직접 실행·런타임 실측
> 침습도: **read-only** — 대상 파일·저장소 상태를 변경하지 않았고, 워치독·설치·정화 스크립트를
> 한 번도 실행하지 않았다. 실측은 순수함수 호출·격리 재현·호스트 상태 관측에 한정했다.

---

## 결론

**terraforming_node 의 문제는 "만들지 않은 것"이 아니라 "전파되지 않은 것"이다.**

직전 hint-publisher 감사의 결론은 *"설계는 정확하고 배선이 비어 있다"* 였다. 이 스킬은 다르다.
배선은 대체로 있고 **실제로 돌고 있다** — 하드다운 워치독 3종이 `HB[armed]` 로 살아 있고, 1초 수집기가
지금 이 순간 쓰고 있으며, 자체검사 20종·139케이스가 전량 통과한다. 검증 규율도 이례적으로 높다:
자기 실패를 일부러 유발해 검증기가 그것을 삼키지 않는지 확인하는 **메타시험**, 부재와 0을 가르는
`ABSENT = -1` 센티넬, 원자적 교체+fsync, 위양성 가드를 실측 근거와 함께 **자진 철회**한 이력.

결함은 그 규율의 부재가 아니라 **규율이 한 파일에 갇힌 것**이다. 이 프로젝트가 이미 자기 문서에
*"교훈이 파일 단위로 갇힌다"* 로 명명해 둔 패턴이, 이 스킬에서 가장 순수한 형태로 재현되고 있다.

가장 선명한 세 장면:

- `blackbox_session.py:124-134` 는 상수 중복 문제를 **정본 import + fail-loud 폴백**으로 이미 해결했고,
  주석에 *"옛 판본은 이 두 값을 손으로 적어 두고… 그리고 갈라졌다"* 는 자기 사례까지 기록했다.
  **그 처방이 형제 파일 `agent_guard.py` 로 가지 않았고, 거기서 갈라짐이 3차 재발했다.**
- `verify_node_blackbox.sh:76-85` 의 주석은 *"여기는 특히 치명적이다 — 위음성이면 서빙 중인 노드를
  죽인다. 파이프를 없앤다"* 라고 위험을 정확히 서술하고 SIGPIPE 기전을 제거했다. **같은 줄에 남은
  `2>/dev/null` 이 동일한 위음성을 내는 두 번째 기전이다.**
- `agent_guard.py:110` 의 주석은 *"데몬과 같은 값"* 이라고 **단언**한다. 설치된 실물 대비 거짓이다.
  단언이 검증을 대체하면, 그 단언이 깨진 순간을 아무도 모른다.

그리고 이 감사는 정적 감사만으로는 볼 수 없었을 것을 하나 잡았다 — **`envelope.json` 은 한 번도
존재한 적이 없다.** 매일 그것을 만들어야 할 서비스가 배포 이래 연속 실패해 왔고, 그 실패가 관측
평면에 도달하지 않는다. 코드는 fail-loud 였다. **가드는 큰 소리로 울었지만, 듣는 사람이 없는
자리에서 울었다.**

이 판정은 이 스킬의 설계 가치를 부정하지 않는다. 토폴로지 제1축(불변식 A)을 `sub_mode` 1:1 사상으로
**표현 불가능하게** 만든 `node_role_contract.py` 는 이 저장소에서 가장 잘 설계된 타입이며, 회귀핀이
실제 사건 문서를 직접 인용한다. 결함은 그 정교함이 **무형식 dict 와 손으로 적힌 상수 위에 얹혀
있다**는 층위에 있다.

---

## 감사 방법과 증거 등급

전문 에이전트 7종을 서로 다른 렌즈로 병렬 투입하고, 파급이 큰 주장은 감사자가 직접 재현해 확증했다.

| 등급 | 뜻 |
|---|---|
| **실측** | 감사자가 이 호스트/저장소에서 직접 실행·관측해 확인 |
| **대조** | 감사자 또는 에이전트가 코드를 직접 읽어 확인 |
| **정적** | 에이전트의 코드 독해 결과 — 감사자가 재실행하지 않음 |

투입 에이전트: `code-reviewer` ×2(계약 코어 / 렌더·교환) · `silent-failure-hunter` ×2(blackbox Python /
셸 전량) · `comment-analyzer` · `pr-test-analyzer` · `type-design-analyzer`.

### 방법론 주의 — 이 환경의 `grep` 은 ugrep 이다

`/usr/bin/grep` 은 **ugrep 7.8.4** 이며 `.gitignore` 를 기본 적용한다. 평범한 `grep -rn` 이 무시대상
파일을 **조용히 건너뛴다** — 부정 주장("리더가 없다")을 확정하는 데 쓰면 위양성이 난다.

실측으로 영향 범위를 확인했다: 대표 질의에서 기본 grep 7건 vs `--no-ignore-files` 15건. 가려진 8건은
**전부 진짜 gitignored**(`seed/` 백업 사본 7 + ignored plan 1)였고 **추적 파일 은닉은 0건**이라,
이번 감사의 부정 주장은 유효하다. 다만 함정 자체는 실재하므로 기록한다.

### 감사 기준선 (실측)

| 항목 | 값 |
|---|---|
| 워크스페이스 | 2026-08-31 22:49 **fresh clone** (reflog 단 1줄) |
| `manifest.yaml` | **부재** (비추적·미테라포밍). `manifest.template.yaml` 만 존재 |
| 구문 검사 | Python 전량 `py_compile` 통과 · 셸 전량 `bash -n` 통과 |
| 자체검사 | 20종 보유 / 순수 6종 직접 실행 **139/139 PASS** |
| 중앙 회귀 하네스 | `runtime_selftest.py` 는 hint-publisher 전용 — terraforming 0건 |
| 실제 집행기 | `verify_distribution.py`(자체검사 2종) · `verify_node_blackbox.sh`(10종) · `claim_predicates.py`(14 바인딩) |

### 살아있는 것 — 하드다운 1차 방어는 건강하다 (실측)

| 유닛 | 상태 |
|---|---|
| `…-blackbox-collect` | active/running · 1초 샘플러 정상 기록 중 |
| `…-blackbox-watchdog` | active/running · `HB[armed]` |
| `…-blackbox-thermal` | active/running · `HB[armed]` · `stale=0` |
| `…-memwatch` | active/running · `HB` 정상 |
| **`…-blackbox-lifecycle`** | **failed (exit 1) · 2026-08-28~09-01 연속 5일 전부 실패** |

### 선언 vs 실재 — `docs/logs/` 데이터 평면 (실측)

`.claude/rules/docs.md` §기계판독 데이터 평면이 선언한 6종 중 **데이터를 담은 것은 2종**이다.

| 경로 | 상태 |
|---|---|
| `samples/` | 있음 (신선 — 지금 기록 중) |
| `events/` | 있음 (2026-07·08 · journald 재수확분) |
| `rollup/` | **빈 디렉터리** |
| `envelope.json` | **부재** — docs.md 가 *"에이전트가 폴링마다 읽는 유일한 파일"* 이라 선언한 것 |
| `capture_verified.json` | **부재** — docs.md 가 *"상태 권위(`installed` 아님)"* 라 선언한 것 |
| `seed/` | **부재** |

---

## 심각도 1 — 8건

### ① `agent_guard` 가 데몬과 다른 상한으로 판정한다 — 정상 로드 사살 조건이 살아 있다

**`scripts/node_blackbox/agent_guard.py:110` · `:145`** · 증거 **실측** · *3개 에이전트 독립 수렴*

1단 에이전트 가드가 상한 상수를 손으로 박아 두었고, 정본은 2026-08-18 에 개정됐다. 이 호스트에
실제 설치된 `/etc/easy-vllm/eta_params.env` 를 읽어 3층을 대조했다.

| 층 | `decl_margin` | `min_ceiling` | 수락하는 최소 선언바닥 |
|---|---|---|---|
| `blackbox_eta.DEFAULTS` (정본) | 3072 | 8192 | — |
| **데몬 실효** (설치된 env) | **3072** | **8192** | **11,264 MiB** |
| `agent_guard.py` (하드코딩) | 8192 | 16384 | **24,576 MiB** |

**실명 창 `[11264, 24576)`** — 이 구간의 예산 선언을 **데몬은 존중하고 가드는 "선언 없음"으로
버린다.** hy3 캠페인 실측 선언바닥 13,801 이 정확히 이 창 안이다.

선언이 버려지면 `arm_ceiling = INF` 가 되고, `classify()` 의 억제절 `if mem_avail > arm_ceiling:
return NORMAL` 이 **결코 참이 되지 않아** 판정이 ETA 규칙으로 떨어진다 →
`NOTIFY → ACT → LAST_RESORT` → `docker stop`. **운영자가 선언한 예산 안에서 정상 로드 중인
컨테이너가 사살된다.**

두 주석이 실물 대비 **둘 다 거짓**이다:
- `:110` *"데몬 `BB_DECL_MARGIN_MIB` 와 **같은 값** — 두 층이 다른 상한을 쓰면 안 된다"*
- `:145` *"**데몬과 동일한** 하한 가드"*

**역설**: 이 결함은 `eta_params.env` 가 **존재할 때만** 나타난다. 파일이 없으면 셸 내장 기본값이
8192/16384 라 두 층이 우연히 일치한다. 즉 **제대로 설치된 노드에서만 가드가 틀린다.**

**3차 재발이다.** 이 함수의 docstring `:115-121` 이 스스로 기록한다 — *"이 가드는 처음에 이 함수
없이 만들어졌고, 그 결과 ⑥의 정상 재기동을 4초 만에 죽였다. 데몬은 같은 결함을 오늘 아침에
고쳤는데 그 교훈이 가드로 전파되지 않았다."* 그 재발 방지로 붙인 주석이 지금 거짓이다.

**자체검사가 원리적으로 못 잡는다** — `:452` 는 `== 45352 - DECL_MARGIN_MIB` 로 **자기 상수를
기준으로 자기를 검증**하고(역-오라클), `:460` 은 갈라진 동작을 PASS 로 못박아 두었다.

**처방이 이미 저장소 안에 있다** — `blackbox_session.py:124-134` 가 정본 import + fail-loud 폴백 +
`_WD_CONST_SOURCE` 출처 표기로 같은 문제를 해결했다. 옮기기만 하면 된다.

---

### ② 그라운딩 하드게이트가 오타 한 글자로 열린다 — 헌법 불변식 B 의 유일한 기계 집행점

**`scripts/library_exchange.py:285-340`** · 증거 **실측** · *2개 에이전트 독립 수렴*

`receive()` 는 리포트를 `isinstance(report, dict)` 로만 보고, 인용 의무 판정을 **서브가 저작한
`phase`·`status` 두 문자열에만** 의존한다. 바로 옆에 있는 `sub_node/task-report.schema.json` 이
두 필드를 `required` + `enum` 으로 못박고 서브로 복제까지 되는데, **판정기가 그것을 한 번도 쓰지
않는다.**

| 입력 | `accepted` |
|---|---|
| `phase=build` + `status=completed` (대조군) | **False** — `GROUNDING_EXCHANGE_ABSENT` 로 정확히 차단 |
| `phase` 키 삭제 | **True** |
| `phase` 오타 `"buildd"` | **True** |
| `status` 대문자 `"Completed"` | **True** |
| `{}` (빈 오브젝트) | **True** |
| `{"hello":"world"}` | **True** |

전수 grep 확인: `task-report.schema.json` 을 **로드해 검증하는 코드는 0건**이다.
`render_sub_env.py:245` 는 복제만, `sync_to_sub.sh:1161` 은 md5 대조만 한다. 136행짜리 정밀 계약이
저장소에 있고 서브로 복제되고 무결성 검사까지 받지만, **그것에 대해 아무것도 검증되지 않는다.**

`_extract_report()`(`:426-445`)가 경로를 더 넓힌다 — 에이전트 출력의 첫 `{` ~ 마지막 `}` 를 관대하게
잘라오므로 잡문 속 아무 JSON 오브젝트나 "리포트"가 된다.

헌법 불변식 B 는 *"인용 없는 결정은 누락이며, **누락은 기계가 fail-closed 로 잡는다**"* 이다.
그 기계가, 하필 **LLM 이 저작한 JSON** 의 오타 한 글자로 열린다. 서브는 LLM 이다.

저자는 fail-closed 를 의도했다 — `receive("not-an-object")` 는 `REPORT_UNREADABLE` 로 정확히
막는다(`:316-320`). **"dict 이지만 task-report 가 아닌 것"** 이 그 사이로 빠진다.

---

### ③ 3자-일치 단언이 브랜치 미해소 시 통째로 증발한다 — Flag 발급 fail-open

**`scripts/scan_node.py:531-538` · `:485` · `:932-936`** · 증거 **실측**

§1.5 3자-일치 단언 두 다리가 모두 `if branch_topo and …` 로 감싸여 있어, git 브랜치가
`single-node|multi-node` 로 해소되지 않으면 **단언 전체가 no-op** 된다. 그런데 emit 블록은 그와
무관하게 `branch_verified: true` 를 상수로 기입한다(`:485`).

| 케이스 | `consistent` | `mismatches` | status |
|---|---|---|---|
| A1 `declared=single` / `branch=None` / `mani=None` | True | `[]` | ok · exit 0 |
| **A2 `declared=single` / `branch=None` / `mani=multi`** | **True** | **`[]`** | **ok · exit 0** |
| A3 `declared=single` / `branch=multi` (대조군) | False | 검출 | **blocked · exit 2** |

**A2 가 결정적이다** — manifest 가 `multi` 라 선언했고 사용자는 `single` 을 선언한 **정면 모순**인데
통과한다. 코드가 `if mani_topo and branch_topo and …` 라서 **manifest 는 브랜치를 경유해서만
검사되기 때문**이다. 헌법이 *"manifest 가 단일 권위 — git 브랜치로 추론하지 않는다"* 라 못박은
관계가, 실제로는 **브랜치가 있어야만 manifest 가 검사되는** 형태로 뒤집혀 있다.

**발화 조건**: detached HEAD(`git checkout <tag>`) · 사용자 정의 브랜치명 · `.git` 없는 배포본 사본
(헌법 §목표 *"배포 단위 = 스켈레톤"*).

**파급**: `branch_verified` 를 읽는 곳은 5개 스킬에 걸쳐 있고 — `manifest_contract.py:116` ·
`staleness_gate.py:370` · `recipe.py:353` · `render_dockerfile.py:1022` · `scan_node.py:738` —
**전부 `is not True` 로 저장된 불린을 그대로 신뢰한다. 재검증자는 0이다.** 리터럴로 박혀 발행된
`true` 하나가 4개 하류 스킬의 진입 게이트를 연다.

`_self_test` 9개 게이트 케이스가 **전부 `branch_topo` 를 명시 전달**해, `None` 을 치는 케이스가
하나도 없다.

---

### ④ MemAvailable 판독 실패 → 워치독이 exit 0 으로 조용히 사라진다

**`scripts/node_blackbox/mem_watchdog_eta.sh:493`** · **`scripts/host_safety/mem_watchdog.sh:46`**
· 증거 **실측**(격리 재현 — 실제 워치독 미실행)

```bash
mem=$(( $(awk '/MemAvailable:/{print $2}' /proc/meminfo) / 1024 ))
```

awk 출력이 비면 `$(( / 1024 ))` 가 산술 syntax error 를 낸다. bash 5.2 에서 이 오류는 셸을 죽이지
않고 **`while` 루프를 이탈**시킨다. 두 파일 모두 루프가 `sleep "$INTERVAL"` / `done` 으로 끝나고
**뒤에 코드가 없으므로** 프로세스는 **exit 0** 으로 끝난다.

격리 재현 결과 — 폴 로그가 **한 줄도 찍히지 않고** 첫 폴에서 즉시 이탈, 최종 `exit=0`.

- systemd 인스턴스는 `Restart=always` 로 5초마다 재기동→즉시 이탈을 반복하고,
  `verify_node_blackbox.sh:165` 의 `systemctl is-active --quiet` 는 그 순간을 잡아 **✓ active** 를 찍을 수 있다.
- 더 나쁜 쪽은 **하네스 협역 인스턴스**다. `host_safety/mem_watchdog.sh` 는 systemd 없이
  `run_trial`·`multinode_serve_smoke.sh` 가 띄운다. 감독자가 없으니 exit 0 을 정상 종료로 받고,
  **그 trial 은 워치독 0층으로 끝까지 간다.**
- 로그 한 줄도, 이벤트도, 비-0 종료코드도 없다. 4종 판정표 **폴백·결함** + 막힘 3분류 **침묵 누락**.

관련: `INTERVAL="${2:-1}"` 에 검증이 없어 `0` 이 들어가면 `rate=$(( … / 0 ))` 로 같은 경로에
떨어진다(`:78`).

---

### ⑤ 강제 패닉 게이트가 docker 불통 시 열린다 — 남의 서빙이 도는 노드를 죽인다

**`scripts/node_blackbox/verify_node_blackbox.sh:82-85`** · 증거 **실측**(게이트 로직만 격리 재현)

이 감사에서 가장 인상적인 발견이다. 같은 블록의 주석 `:76-81` 이 위험을 정확히 서술한다:

> *"★ pipefail 하에서 `producer | grep -q` 는 금지다… **여기는 특히 치명적이다 — 위음성이면
> 서빙 중인 노드를 죽인다.** 파이프를 없앤다."*

저자는 SIGPIPE 기전을 제거했다. **같은 줄에 남은 `2>/dev/null` 이 동일한 위음성을 내는 두 번째
기전이다.**

| docker 상태 | 게이트 |
|---|---|
| 정상 + vLLM 가동 (대조군) | 거부 · exit 3 ✓ |
| 데몬 정지 | **통과 → 패닉 진행** |
| 미설치 | **통과 → 패닉 진행** |
| 소켓 권한 없음 | **통과 → 패닉 진행** |

`sudo -n` 컨텍스트에서 docker 그룹이 안 붙거나 데몬이 재시작 중이면, 남의 서빙이 도는 노드에
`echo c > /proc/sysrq-trigger` 가 들어간다. **되돌릴 수 없다.**

**한 원인을 고치고 바로 옆의 다른 원인을 남긴** 형태다.

---

### ⑥ `docker kill` 이 실패해도 `kill_ack` 를 무조건 발행한다 — 블랙박스 위조

**`scripts/node_blackbox/mem_watchdog_eta.sh:246-247`** ·
**`scripts/node_blackbox/thermal_watchdog.sh:201-202`** ·
**`scripts/host_safety/mem_watchdog.sh:57-58`** · 증거 **실측**

```bash
docker kill $ids 2>&1 | sed 's/^/[bb-watchdog] /'
emit_event "watchdog_kill_ack" "\"targets\":\"$ids\""
```

파이프로 넘겨 종료코드를 버리고 다음 줄에서 무조건 ack 를 남긴다. 재현 결과, `docker` 가 exit 1
이어도 이벤트 평면에는 `watchdog_trip` + `watchdog_kill_ack` 가 나란히 남는다.

**시나리오**: 데몬 정지·소켓 권한 상실·containerd 무응답 → 컨테이너는 살아 있고 호스트는
하드다운. 사후 분석에 남는 유일한 기록이 **"워치독이 트립했고 kill 을 완료했다"** 다.
실패 문자열은 stdout/journald 로만 흘러 이벤트 평면에 오지 않는다.

이 파일들은 `mode` 필드를 넣으면서까지 *"죽인 것인가 관측인가를 데이터에서 가른다"* 고 선언한다.
**가르지 못하는 건 dry-run 이 아니라 실패다.**

---

### ⑦ `envelope.json` 은 한 번도 존재한 적이 없다 — 4단 결함 체인

증거 **실측** · *정적 감사로는 볼 수 없는 발견*

`easy-vllm-blackbox-lifecycle.service` 가 **2026-08-28부터 09-01까지 연속 5일 전부 실패** 중이다.
journald 메시지는 매번 동일하다: `ModuleNotFoundError("No module named 'blackbox_eta'")`.

1. **설치기가 sibling import 를 깨뜨린다** — `install_node_blackbox.sh:187,190` 이
   `blackbox_eta.py` → `easy-vllm-bb-eta`, `regen_envelope.py` → `easy-vllm-bb-regen-envelope` 로
   **확장자를 떼고** 설치한다. 그런데 `regen_envelope.py:47` 은 `from blackbox_eta import …` 로
   같은 디렉터리의 `.py` 를 찾는다. 설치본과 소스본은 diff 동일(3/3 확인) — **순수 패키징 결함**이다.
   `:50` 의 `# 배치 오류는 조용히 넘기지 않는다` 대로 **코드는 정확히 fail-loud 했다.**
2. **verify 가 사정거리 0** — `verify_node_blackbox.sh:149` 는 설치 매핑의 존재/동일성만 보고,
   `:129` 의 `--self-test` 는 **소스 디렉터리에서** 돌아 sibling 이 있으니 통과한다.
   **설치본을 한 번이라도 실행해보는 검사가 없다.**
3. **실패가 관측 평면에 도달하지 않는다** — `blackbox_events.py:322-324` 의 `DEFAULT_SOURCES` 는
   watchdog·thermal·memwatch **3종뿐**이고 `lifecycle` 이 없다. 매일의 실패가 journald 에만 남는다.
4. **결과** — `envelope.json` 부재. 그리고 `agent_guard.py:82-92` 가 읽는 kill 계약 3필드
   (`threshold_mib`·`latency_upper_bound_s`·`daemon_kill_s`)가 **항상 리터럴 기본값**(10240/14/6)인데,
   기동 배너 `:281-284` 는 `src=<envelope 경로>` 를 출력해 **계약이 파일에서 로드된 것처럼 보고**한다.
   `LAST_RESORT` 경계가 항상 `6+14+10=30s` 로 굳는다.

**가드는 큰 소리로 울었다. 듣는 사람이 없는 자리에서 울었을 뿐이다.**

부수 실측: `regen_envelope` 이 읽는 3키를 **정본 재생성기가 발행하지 않는다**(v2 스키마는
`kill.latency_s.{max,p50,n,scope}` 를 쓴다). 즉 lifecycle 을 고쳐도 ④의 키 불일치가 남는다 — **두
결함이 직렬로 겹쳐 있다.**

---

### ⑧ `budget_renew_loop` 이 docker 불통을 "컨테이너 부재"로 읽고 exit 0

**`scripts/node_blackbox/budget_renew_loop.sh:43`** · 증거 **정적**

```bash
[ -n "$(docker ps --filter "name=^${1}$" --filter status=running -q 2>/dev/null)" ]
```

`2>/dev/null` 이 **"부재"와 "판단 불가"를 같은 빈 문자열로 뭉갠다.** 상주 서빙이 도는 중 docker
데몬이 몇 초 재시작하면 사이드카가 **정상 종료**로 사라지고 → `serve_budget.env` 만료 →
`BB_ARM_CEILING_MIB` 무한대 복귀 → **선언된 바닥 없는 옛 규칙**으로 상주 서빙 감시.

이 파일 헤더가 *"없애려는 것이 정확히 그 무보호 구간"* 이라 적은 상태가 재생성된다. W-7 의 존재
이유로 든 "18시간 만료 방치"의 재발 경로가 **자기 안에** 있다.

---

## 심각도 2 — 주요 발견

| # | 위치 | 결함 | 증거 |
|---|---|---|---|
| ⑨ | `CLAUDE.md:58` · `SKILL.md:301,553` | **헌법 불변식 A 의 외부 그라운딩 인용이 끊겼다.** 커밋 `d9fbdfb` 의 report 13건 rename 때 인용부가 따라가지 않아 `docs/report/` 참조 2종이 부재 | **실측** |
| ⑩ | `sub_node/CLAUDE.template.md:12,20,26,29` | single 로 렌더해도 서브 페르소나가 *"너 = slave(Ray worker)"* 이고 **브랜치로 토폴로지를 분기하라**고 지시. 같은 렌더의 `Agent_Card.json` 은 `sub_mode:"a2a-agent"` — **불변식 A 와 브랜치 추론 금지 동시 위반**. W-3 정정이 카드에만 적용됐다 | **실측** |
| ⑪ | `sub_node/comms.md:48` | 서브 build 성공술어로 `byte-equiv` 요구. SKILL.md `:316-321` 이 *"digest 를 판정 기준으로 쓰면 정상 배포를 불일치로 오판한다"* 며 명시 반증한 기준. 성실한 서브가 정상 빌드를 실패로 보고한다 | 대조 |
| ⑫ | `smoke_clone.sh:26-32` + `.claude/pii_terms.txt` 부재 | **PII 게이트가 `(192\.168\.)` 단 1패턴으로 조용히 축소**된다. `abs-op-path`·`email`·`spark-host`·기타 사설대역 전부 무검사. 그 결과 서브 배달 대상 3파일에 운영자 절대경로가 박힌 채 남아 있다 | **실측** |
| ⑬ | `scan_node.py:463-517` | emit 이 `nodes:`·`interconnect:` 를 통째로 재작성하며 운영자 저작 필드를 소실시킨다 — single 은 `nodes: []` 하드코딩(등록된 서브 소멸), multi 는 `hw_verified`·`mtu` 등 누락. 같은 출력 하단은 nodes 에 **손으로 더하라**고 지시한다 | **실측** |
| ⑭ | `scan_node.py:476-477` | `scanned_at` 이 `datetime.now()` 벽시계. docs.md 가 *"시각은 `--now` 주입만"* 이라 못박고 그 선례로 `staleness_gate` 를 지목했는데, **그 축의 유일한 입력을 만드는 자리**가 벽시계다 | **실측** |
| ⑮ | `staleness_gate.py:71` vs `scan_node.py:853-871` | 노드축 `driver_version` 비교에 **생산자가 없다**. scan 이 안내하는 대로 운영자가 손으로 기입하면 **영구 HW_DRIFT 위양성**(exit 4)이 되어 재테라포밍을 무한 요구 | **실측** |
| ⑯ | `install_node_blackbox.sh:395-454` | L3 의 파괴적 명령 10개가 **dry-run 에 안 보인다**(`run()` 미사용). SKILL.md §2.6 이 *"dry-run 선행 고지"* 로 HITL 게이트를 걸었는데 **승인 대상이 실제 실행분보다 작다**. 자매 `purge_host_safety.sh` 는 전부 노출한다 | 대조 |
| ⑰ | `install_node_blackbox.sh:160,460` | 열·전력 계층이 **전제 확인과 설치직후 검증 양쪽에 없다**(5종 중 thermal 만 누락). 기동 실패해도 **INSTALL PASS**. `verify` 는 판정하는데 **설치자만 눈이 없다** | 대조 |
| ⑱ | `install_node_blackbox.sh:449-475` | `update-grub` 결과 미확인 + **성공 문구 무조건 출력**. grub.cfg 백업 실패도 삼키고 존재하지 않는 백업 경로를 안내. kdump 무장 해제 실패도 판정에 미반영 | 대조 |
| ⑲ | `blackbox_collect.py:211-222` | nvidia-smi 서브프로세스가 죽어도 `proc.poll()` 을 안 봐서 **마지막 값이 영구히 실측인 척**한다. CSV 에 신선한 `ts` + 얼어붙은 값이 기록되고, thermal 워치독의 유일한 stale 방어(`now - GPU_TS`)를 **정확히 비껴간다** | **실측** |
| ⑳ | `logs_lifecycle.py:169-265` | 압축 실패에 예외 처리가 없어 **디스크 만재 시 3·4단계(나이·용량 삭제)가 호출조차 안 된다.** 이 파일의 존재 이유가 "블랙박스가 스스로 사고 원인이 되는 자가당착 방지"인데 **정확히 그 조건에서만 무력해진다** | 대조 |
| ㉑ | `logs_lifecycle.py:224-260` | 삭제·압축이 **기록보다 먼저** 일어난다. `append_event` 가 터지면 파일은 지워졌고 기록은 없다 — docs.md §침묵 삭제 금지를 순서 하나로 깬다 | 대조 |
| ㉒ | `node_identity.sh:63-90` | 깨진 `Agent_Card.json` 또는 읽기 불가 manifest → **서브가 자기를 `main` 으로 해소**(rc=0). 같은 파일 `:30-32` 가 *"두 노드 로그가 같은 이름으로 겹친다(복구 불가한 혼합)"* 고 경고한 그 상태 | **실측** |
| ㉓ | `render_sub_env.py:421-451` | `_copy_tracked` 가 git 실패를 통째로 삼키고 **os.walk 전량 복제로 폴백**한다(경고 0줄). docstring 의 *"사적/생성물 자동 제외"* 보장이 깨지고, 서브 `.gitignore` 가 유입을 기록조차 못 한다 | 대조 |
| ㉔ | `sync_to_sub.sh:1160-1169` | `verify_checksums` 가 `node_blackbox/` 16파일과 docs 스켈레톤 5종을 **검증 목록에서 뺀다**. 부분 배달이 "✅ 전부 통과"로 끝나고 서브 설치가 부재 파일에서 죽는다 | 대조 |
| ㉕ | `render_sub_env.py:278-290` + `sync_to_sub.sh:1204` | A2A 위임 키에 **회수 경로가 없다**. `hw_verified` 를 내리면 sync 자체가 exit 10 으로 막혀 **폐기를 배달할 수 없고**, 오버레이는 `--delete` 없는 additive 라 키가 서브에 영구 잔존한다 | 대조 |
| ㉖ | `library_exchange.py:269-273` | Freshness 가 **도서관 파일을 열지 않는다**(`hashlib` import 0). 인용된 문서가 실제로 바뀌어도 초록불 — 잡히는 건 서브가 digest 를 고의로 바꾼 경우뿐 | 대조 |
| ㉗ | `manifest_contract.py:52-122` | 통로 topology 와 manifest 선언 topology 를 **대조하지 않는다**. 같은 manifest 가 통로에 따라 TP 1↔2 로 갈리고 **둘 다 Flag valid**. `staleness_gate.py:102` 는 정확히 이 대조를 한다 | **실측** |
| ㉘ | `manifest_contract.py:122` | 필수 필드 검사가 truthiness 뿐 — `gpus_per_node` 가 `'abc'`·`-3`·`True` 여도 Flag valid 이고 TP 가 조용히 1 로 접힌다(`0` 만 거부). `roofline.py:222` 가 *"TP 과소평가는 느린 서빙을 통과시킨다"* 고 적은 경로 | **실측** |
| ㉙ | `thermal_watchdog.sh:410-425` | stale 판정이 **"값을 읽었는데 낡은 경우"에만** 성립. CSV 파일 부재(수집기 사망 · **UTC 자정 롤오버 직후**)면 `stale=0` 이고 로그·이벤트 **0** — 가장 큰 실명 케이스에서 침묵 금지 계약이 깨진다 | 대조 |
| ㉚ | `mem_watchdog_eta.sh:138` · `thermal_watchdog.sh:95` | 이벤트 기록 실패를 `|| true` 로 삼킨다. 바로 다음 6줄이 *2026-08-01 EACCES 사고*를 서술하며 `chown` 을 넣는데, **그 사고의 증상(기록 유실)을 감추는 `|| true` 는 그대로**다 | 대조 |
| ㉛ | 전 워치독 | **정지 경로가 없다** — `trap`·`ExecStop` 0건, `*_start` 이벤트는 있고 `*_stop` 은 없다. "워치독이 돌았는데 못 잡았다"와 "3시간 전에 사라졌다"를 **데이터로 가를 수 없다**. ④와 결합하면 최악 | **실측** |
| ㉜ | `verify_node_blackbox.sh:475-502` | `capture_proven` 이 파싱 실패 시 조용히 `null` 로 **다시 쓰인다**. 이 절의 헤더가 근거로 든 2026-07-31 덮어쓰기 사고의 **다른 경로가 그대로 남았다**. 쓰기 실패해도 "기록: …" 출력 | 대조 |
| ㉝ | `install_node_blackbox.sh:380` · `verify_node_blackbox.sh:342` | 토폴로지를 **`output/multi/manifest.yaml` 파일 존재로 추론**한다. 브랜치 전환이 working copy 를 유지하므로 잔존 파일이 단일 노드에 netconsole 을 설치하게 만든다 — 헌법 §브랜치 추론 금지의 우회 | 대조 |
| ㉞ | `install_netconsole.sh:46-51` | 자기역할 판정이 **IP 부분문자열 매칭**. 옥텟이 접두 관계인 제3 노드에서 불일치 fail-loud 가 발동하지 않고 오판한다. `node_identity.sh:82` 는 *"접두 오인 방지"* 를 명시 — 그 규율이 이 파일에 안 왔다 | **실측** |
| ㉟ | `render_sub_env.py:443-450` | `shutil.copyfile` 이 모드를 보존하지 않아 런타임블럭 실행파일 12개가 **0664 로 배달**된다. 같은 렌더러가 다른 두 계열에는 명시 `chmod` 를 건다 | **실측** |

---

## 심각도 3~4 — 계열별 요약

개별 나열 대신 계열로 묶는다. 총 40여 건이며 전수는 각 에이전트 원본에 있다.

| 계열 | 대표 사례 | 건수 |
|---|---|---|
| **도달 불가 분기** | `staleness_gate.py:394` 미래-timestamp 노트(선행 게이트가 이미 early return) · `blackbox_thermal.py:626` `--set` 이 정수 가드보다 먼저 절삭해 가드를 무력화 · `node_identity.sh:77` 값이 상수라 "갈리면 fail-loud" 가 공허하게 참 | 6 |
| **no-op 옵션·죽은 값** | `manifest_contract.py:326` `--require-flag` 삼항식 양변 동일 · `agent_guard.py:18` 문서화된 exit 4 경로 0개 · `scan_node.py` `cross_validation` 산출 후 소비자 0 · `budget_renew_loop.sh:101` 아무도 안 읽는 마커 | 8 |
| **하드코딩 총계·상수 중복** | `node_role_contract.py:467` `total = 24` · `library_exchange.py:688` `total = 30`(둘 다 `len(cases)` 아님) · `999999999` 가 두 개념으로 10곳 | 5 |
| **종료코드 계약 분열** | `exit 2` 가 사용오류/자체시험실패/필드누락/정상중단 4의미 · `install_node_blackbox.sh` 만 1↔2 뒤바뀜 · `purge_host_safety.sh` 는 2를 안 씀 · 호출부 2곳이 `2>/dev/null` 로 전부 boolean 붕괴 | 7 |
| **문서 수치 드리프트** | `SKILL.md:654` 케이스 수 3종 낡음(21→25 · 34→38 · 17→30) · §2.2 배달 표가 `node_blackbox` 16파일·위임키 통째 누락 · "10아티팩트" vs 실제 40+ | 9 |
| **`--help` / 사용법 어긋남** | `install_node_blackbox.sh:80` `sed -n '2,26p'` 가 예시·종료코드 잘라먹음 · `purge_host_safety.sh:21` `--seed-dir <path>` 공백형 미파싱(실제는 `=` 형만) · `mem_watchdog_eta.sh:64` 없는 `exit 1` 경로 | 6 |
| **주석이 폐지된 처방 인용** | `install_host_safety.sh:120-129` kdump 처방 10줄(바로 다음 줄이 "폐지") · `blackbox_thermal.py:130` *"조용히 버리지 않는다"* 인데 `:133` 이 조용히 필터링 | 5 |

---

## 관통하는 패턴 3종

### 1. 주장과 검증의 분리 실패

부작용 명령 바로 뒤의 성공 문구가 **조건 없이** 출력된다 — `docker kill` → 무조건 `kill_ack`(⑥) ·
`update-grub` → 무조건 "✓ 갱신"(⑱) · `> "$OUT"` → 무조건 "기록:"(㉜).

### 2. "부재"와 "판단 불가"의 융합

`2>/dev/null` 이 docker 불통(⑤⑧) · 읽기 불가 manifest(㉒) · CSV 부재(㉙) · grub.cfg 부재를 전부
**"없음 = 안전"** 으로 접는다. 이 저장소는 *"읽지 못한 것과 없는 것은 다르다"* 를 **주석으로 네 번**
적어 뒀고, 코드에서 네 번 어겼다.

### 3. 단언이 검증을 대체했다

심각도 1·2 의 절반 이상이 **"완료·동일·전부·반드시" 형태의 단언문**에서 나왔다 —
`✅ 배선 완료` · `데몬과 동일한` · `만든 것 전부` · `반드시 명시 검사한다` · `조용히 버리지 않는다`.

이 프로젝트의 문서는 불변식을 단언하는 습관이 강하다. 그 자체는 미덕이다. 그러나 **단언이
그것을 검사할 이유를 없애면, 단언이 깨진 순간을 아무도 모른다.** ①이 정확히 그렇게 실패했다.

처방은 단언을 줄이는 것이 아니라, **단언마다 그것을 깨뜨리면 빨간불이 켜지는 지점을 하나씩
붙이는 것**이다. 이 스킬 안에 이미 좋은 선례가 있다 — `mem_watchdog_eta.sh:314-322` 의
**메타시험**(일부러 FAIL 을 유발해 검증기가 그것을 삼키지 않는지 확인)과, `blackbox_thermal.py`
자체검사가 수집기 상수와 셸 내장 기본값을 **리터럴 대조**하는 교차검증 술어다.
**후자가 `agent_guard` 에만 없어서 ①이 발생했다.**

---

## 검증 커버리지 — 게이트 28개 판정

`A`=동작 검증 있음 · `B`=정적 리터럴만 · `C`=검증 없음 · `D`=있으나 무의미

- **A 판정 18개** — §0.5 emit fail-closed · §1.4 bw-floor · §1.5 3자일치 · A2A 키 발급 ·
  배달평면 판정 · 그라운딩 게이트 · staleness 3축 · ETA 트립 · L3 proof-of-capture 등
- **B 판정 3개** — §1.7 work_dir 프로비저닝(구간 순서 앵커만) · 무인 sudo 금지 · 협역 워치독
- **C 판정 5개** — 무단 스캔 금지(코드 백스톱 없음) · §2.5 model-less 카나리(**구현체 부재**) ·
  §2.6.1 지시서 tripwire · placeholder fail-loud · node_id 해소(배선 0)
- **D 판정 2개** — `no-sub-registered → dormant` 가 **값이 아니라 출처만** 단언(변이 통과 실증) ·
  Flag 하류 게이트(로컬 환경에서 도달 불가 — fresh-clone 하네스로 명시 위임되어 정당)

**가장 위험한 공백 3종**:

1. **설치자·정화자 4종 1,106행에 동작 검증 0** — `purge_host_safety.sh --require-seed` 게이트는
   *"복구 불가한 저널"* 을 지키는데 발화 여부를 아무도 시험하지 않는다.
2. **`host_safety/mem_watchdog.sh` 무검증** — `policy:HOST_SAFETY_LAYERED_DEFENSE.C1` 이 요구하는
   정본이고 `multinode_serve_smoke.sh` 가 부재 시 exit 2 로 죽는 실물 방어층인데, 자체검사 0에
   리터럴 3개만 본다. **형제 `mem_watchdog_eta.sh` 는 최고 수준인데 정본으로 남기로 한 쪽이 0이다.**
3. **자체검사 6종이 어떤 집행기에도 배선되지 않음** — 그 안에 **헌법 불변식 A 의 집행점**
   (`node_role_contract`)과 **불변식 B 의 집행점**(`library_exchange`)이 있다.
   `verify_distribution.py:612-620` 이 스스로 적은 기준이 이것이다:
   *"자체검사에 호출자가 없으면 그것은 L2 가 아니라 L1(산문)이다."*

---

## 타입 설계 등급

| 축 | 등급 | 근거 |
|---|---|---|
| 캡슐화 | **2 / 5** | 실질 클래스 2개뿐. manifest 가 원시 dict 로 흘러다니며 소비자마다 `.get(k) or <기본값>` 재해석 → Flag 게이트 3벌 · `manifest_tp` 3벌 · 스키마 검증기 2벌 · 슬러그 정규식 5벌 |
| 불변식 표현력 | **3 / 5** | `sub_mode` 1:1 사상과 `rank=None` + `not-applicable:` 출처는 **모범**. 그러나 `provenance`/`*_source` 가 선택 필드이자 **객체 스코프**라 "측정인 척하는 상수"가 타입 수준에서 표현 가능(⑦) |
| 유용성 | **3 / 5** | 강제되는 불변식은 **실제 사건**을 막는다(1-GPU 에 TP=2 · 싱글 서브로 빌드킷 34회 유출 · hostname 로그 분열). 감점은 커버리지 — 선언 필드 ~30개 중 게이트 보증 5개, 리더 0인 것 6개 |
| 강제 | **2 / 5** | 배선된 것과 안 된 것이 반반. `task-report.schema.json`·`Agent_Card.json`·`envelope.json`·`manifest.yaml` 이 **검증 로더 0** |

> **가장 잘 설계된 타입**: `node_role_contract.py` 의 `sub_mode` 스킴. 헌법 불변식 A 가 금지한
> "한 스킴의 `role: sub` 로 Ray 워커와 A2A 에이전트를 덮는 것"을 **표현 불가능하게** 만들었고,
> 소비자 배선(`sync_to_sub.sh:171`)까지 실재한다. 불변식 A 는 이 문으로는 새지 않는다 —
> **다른 문(topology 출처 불일치 · ㉗)으로 샌다.**

---

## 이 감사 자체의 결함 (기록)

감사자가 최초 기준선에서 **`runtime_selftest.py` 에 terraforming 이 0건 → "중앙 검증 없음"** 이라는
방향으로 에이전트들을 편향시켰다. 그 사실 자체는 맞았으나(0건 확인), **도출된 함의가 틀렸다.**

`runtime_selftest.py` 는 이 저장소의 중앙 하네스가 아니다. 실제 집행기는 **`verify_distribution.py`**
이고, 감사자는 그것을 보지 못했다. 그 결과 하마터면 **존재하는 139개 케이스를 지우고, 동시에 진짜
공백(배선 4종 누락·단언 구멍 2건·설치자 1,106행 무검증)을 놓칠 뻔했다.**

직전 hint-publisher 감사가 **정확히 같은 방식으로** 실패했다(grep 하나의 출력을 전수로 취급).
이번에는 `pr-test-analyzer` 에게 *"이 기준선을 적극적으로 반증하라"* 고 명시 지시했고, 그 에이전트가
지시대로 반증했다. **처방이 작동했다.**

교훈은 그대로다 — **부정 주장("~가 없다")은 도구 하나의 출력으로 확정하지 않는다.**
이번 감사에서 그 함정이 한 겹 더 있었다: 이 환경의 `grep` 은 ugrep 이고 `.gitignore` 를 기본
적용한다(§감사 방법 참조).

또한 에이전트 결과 하나를 **감사자가 정정했다**: `purge_host_safety.sh --require-seed` 를 심각도 1
"실행 불가"로 보고받았으나, 직접 확인 결과 **게이트는 기본 활성(`REQUIRE_SEED=1`)으로 정상 작동**한다.
실제 결함은 헤더가 존재하지 않는 `--require-seed` 플래그를 안내하고 `--seed-dir <path>` 공백형이
파싱되지 않는 것으로, **심각도 3** 이 맞다.

---

## 권고 (비용 대비 차단력 순)

| # | 처방 | 비용 | 닫히는 것 |
|---|---|---|---|
| 1 | `agent_guard.py` 의 두 상수를 `blackbox_eta.DEFAULTS` 에서 읽도록(`blackbox_session.py:124-134` 패턴 이식) | **수 줄** | ① — 정상 로드 사살 |
| 2 | `receive()` 진입부에 `task-report.schema.json` 검증 + `REPORT_SCHEMA_INVALID` fail-closed | **5줄** | ② — 불변식 B 우회. 스키마도 검증기도 이미 있고 **연결만 없다** |
| 3 | `evaluate_gate` 에서 `declared` 가 있는데 `branch_topo` 가 None 이면 blocking mismatch | **2줄** | ③ — Flag fail-open |
| 4 | `install_node_blackbox.sh` 가 `.py` 확장자를 유지해 설치하거나 sibling 을 함께 배치 | **1줄** | ⑦ — envelope 5일 연속 실패 |
| 5 | `blackbox_events.py:322` `DEFAULT_SOURCES` 에 `easy-vllm-blackbox-lifecycle` 추가 | **1줄** | ⑦ 3단 — 실패의 관측 평면 도달 |
| 6 | 워치독 `mem` 판독 실패 시 fail-loud + `watchdog_read_fail` 이벤트 | 소 | ④ — 침묵 사망 |
| 7 | `verify_node_blackbox.sh:82` 의 `2>/dev/null` 제거 후 rc 판정 → exit 3 | **1줄** | ⑤ — 강제 패닉 오발 |
| 8 | `docker kill` rc 를 잡아 `kill_ack` / `kill_failed` 분기 | 소 | ⑥ — 블랙박스 위조 |
| 9 | `trap 'emit_event watchdog_stop' TERM INT EXIT` | **1줄** | ㉛ — 정지 기록 부재 |
| 10 | `verify_distribution.py` 에 자체검사 4종 배선(`manifest_contract`·`staleness_gate`·`node_role_contract`·`library_exchange`) | **4줄** | 불변식 A·B 집행점이 산문에서 검증으로 |
| 11 | `manifest_contract` 에 통로↔필드 topology 대조 + `gpus_per_node` 도메인 검사 | **3줄** | ㉗㉘ |
| 12 | `CLAUDE.md:58`·`SKILL.md:301,553` 의 report 인용을 새 파일명으로 갱신 | **3곳** | ⑨ — 헌법 인용 단절 |
| 13 | `smoke_clone.sh` 의 PII 폴백을 정본 4패턴으로 + 배달 3파일의 절대경로 치환 | 소 | ⑫ — 배포 PII |
| 14 | `CLAUDE.template.md`·`comms.md` 를 `{{ SUB_MODE }}` 분기로 정정(W-3 미완 이관 완료) | 중 | ⑩⑪ — 불변식 A 누수 |

**1~9 는 대부분 한 줄에서 수 줄이다.** 이 감사에서도 가장 값싼 조치가 가장 큰 차단력을 갖는다 —
결함이 로직이 아니라 **전파와 배선**이기 때문이다.

---

## 최종 판정

1. **이 스킬의 안전 설계와 검증 규율은 이 저장소에서 가장 높은 수준이다.** 메타시험, `ABSENT`
   센티넬, 원자적 교체, 위양성 가드 자진 철회, 사건 문서를 직접 인용하는 회귀핀, 그리고
   불변식 A 를 표현 불가능성으로 봉인한 `sub_mode` 스킴 — 전부 실제 사고에서 도출된 모범 사례다.

2. **그러나 현행 상태로는 배포 승인할 수 없다.** 정상 로드를 사살할 조건이 살아 있고(①), 헌법
   불변식 B 의 유일한 기계 집행점이 오타 한 글자로 열리며(②), 테라포밍 Flag 가 브랜치 미해소만으로
   발급되고(③), 호스트 방어층이 exit 0 으로 사라질 수 있으며(④), 강제 패닉 게이트가 docker 불통에
   열린다(⑤). 다섯은 각각 독립적으로 **물리 노드 또는 상주 서빙의 손실**로 이어진다.

3. **관측 평면이 5일간 거짓말을 하고 있었다.** `envelope.json` 부재(⑦)는 정적 감사로는 볼 수 없었고,
   `docs/logs/` 선언 6종 중 데이터를 담은 것은 2종뿐이다. 이것은 코드 결함이라기보다 **"설치됨"과
   "작동함"을 가르지 않은 검증 설계의 결과**다.

4. **처방의 대부분은 이미 이 저장소 안에 있다.** ①의 해법은 `blackbox_session.py` 에, ③의 대조는
   `staleness_gate.py` 에, ②의 검증기는 `completion_gate.py` 에 이미 구현돼 있다.
   **필요한 것은 새 설계가 아니라 전파다.**

---

> 이 감사는 read-only 로 수행되었으며 대상 파일과 저장소 상태를 변경하지 않았다.
> 워치독·설치·정화 스크립트는 한 번도 실행하지 않았고, 실측은 순수함수 호출·격리 재현·
> 호스트 상태 관측(systemd·journald·설치본 diff)에 한정했다.
> 실측 항목은 감사 시점(2026-09-01 09:36 KST · 커밋 `d9fbdfb` · 브랜치 `single-node`)의
> 로컬 상태에 대한 관측이다. 정적 항목은 코드 독해 결과로, 재실행으로 확증하지 않았다.
