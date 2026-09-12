# strategy.topology.md — 멀티노드 브랜치 특화 헌법

**topology: multi** · layer: topology · 경로 규약 `*.topology.md` · policy:BRANCH_CONSTITUTION_LAYERING

> 위 한 줄이 이 파일의 **자기선언 헤더**다 — 결정론 술어 `topology_parity.py` 가 이 바이트를 읽어
> 체크아웃 브랜치·`output/multi/manifest.yaml`·활성 캠페인 선언과 대조한다(4자일치). frontmatter 에
> 두지 않는 이유: 인식되지 않는 frontmatter 키의 처분은 **문서화된 동작이 아니고**, 블록 HTML 주석은
> 컨텍스트 주입 전에 제거된다 — 그러면 *술어가 읽는 바이트*와 *에이전트가 읽는 컨텍스트*가 갈린다.
> 본문에 두면 둘이 같은 자리를 읽으므로 갈라질 수 없다.

> **이 파일은 특화층이다.** 같은 경로가 `single-node` 브랜치에는 **다른 내용**으로 존재하며, 브랜치
> 동기화는 이것을 방향과 무관하게 **전파하지 않는다**. 공통층(`CLAUDE.md` ·
> `.claude/rules/{workflow,docs}.md`)이 먼저이고, 이 파일은 그 위에서 **이 토폴로지에서만 참인 것**만
> 말한다. 여기 없는 것: 절차(→ `workflow.md`) · 계약기가 파생하는 값(→ `node_role_contract.py`) ·
> 두 토폴로지의 **비교**(→ `terraforming_node` SKILL.md §2.7.0 · 양쪽이 읽어야 하므로 공통이다).

## 이 브랜치가 무엇인가

`multi-node` 는 **분산추론 전용 배포 라인**이다. 체크아웃은 두 가지를 고른다 — 읽을 헌법 특화층(이
파일)과 산출물 통로(`output/multi/`). **토폴로지 사실의 권위는 여전히 `output/multi/manifest.yaml`
이며 브랜치가 아니다**(공통층 불변식). 브랜치는 필터이고, 둘이 어긋나면 4자일치 술어가 막는다.

## 서브는 Ray 워커다 — 그래서 동기가 필요하다

멀티의 `sub` 는 **Ray 워커**다: 위치가 곧 rank 이고, 집단 연산 ABI 가 동기돼야 하며, 제어 평면은
head 에 종속된다. 정체성 권위는 manifest `nodes[]` 인덱스 + role 이고 `sub_mode` 는 `ray-worker` 다.
**이 문단은 "왜"만 적는다** — 판정·배선의 정본은 `node_role_contract.py`(manifest 파생)와
SKILL.md §2.7.0 이며, 파생값을 여기에 다시 적지 않는다.

- **동기의 이유는 로드 균형이 아니라 집단 연산의 lockstep 정합**이다. 그래서 이 요구는 싱글에 적용될
  이유가 애초에 없다 — 싱글은 collective 에 참여하지 않는다.
- ⚠ **"동일 이미지"가 아니라 "동일 ABI"다.** 노드마다 로컬 빌드를 하므로 image digest 일치는 성립하지
  않는다(2026-08-22 실측: 분산 서빙에 성공한 두 컨테이너의 digest 가 서로 달랐다). 정확히 일치해야
  하는 것은 vLLM git SHA · torch · driver 셋이다. digest 를 커플링 판정 기준으로 쓰면 정상 배포를
  불일치로 오판한다.

## 배달 평면은 active 다

- S2.5 sync 는 이 브랜치에서 **열린다** — `sync_to_sub.sh` 가 dry-run 뒤 tracked runtime·위임키·빌드킷
  산출물을 서브에 배달하고 checksum 으로 검증한다. 절차 정본은 `workflow.md` §상태·owner 계약의
  S2.5 행과 SKILL.md §2.7.5 다. 공통층의 "S2.5 배달분은 메인 S4 커밋 전까지 미검증 후보다" 문단은
  **이 브랜치에서 성립한다**.
- **이미지 save/load 전송은 영구 금지다**(공통층). 가는 것은 빌드킷과 레시피이고, 각 노드가 자기
  이미지를 빌드한다.

## 실패 라우팅 — 이 토폴로지 전용

| 신호 | owner/경로 | gate·출구 |
|---|---|---|
| 분산 런타임 실패(OOM·NCCL·Ray) | `upstream-version-watch` multi reference | 증거 보존 → Model-C |

나머지 라우팅은 공통층 `workflow.md` §실패 라우팅이 갖는다.

## 스모크의 범위

컨테이너 변경은 **이 브랜치 토폴로지의 기능 스모크**(TP ≥ 2 분산 서빙 · health 200 + 추론 1회)를
통과하기 전 완료로 판정하지 않는다. 반대 토폴로지의 스모크를 여기서 요구하지 않는다 — 그 산출물
통로는 이 체크아웃에 없다.
