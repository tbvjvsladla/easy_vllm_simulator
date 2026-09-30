# 오케스트레이션 — 그라운딩 교환 포맷

> terraforming_node 스킬 reference — SKILL.md §2.7.8 의 본문이다. SKILL.md(라우터)는 § 번호·제목·포인터만 든다.
> 이관 전 원문: `git show dcb713a:.claude/skills/terraforming_node/SKILL.md` (plan_26093022 Step 4 — 본문 바이트는 그대로 옮겼다).

## 목차

- 2.7.8 그라운딩 교환 포맷 — 서브 요청 → 메인 색인 → 반출 (신설 2026-08-22 · `plan_26082214` §4.2)
- 세 메시지 (전송은 신설하지 않는다 — 기존 A2A 통로에 실어 보낸다)
- 증거그래프 어휘 대응
- 하드게이트 — 세 질문 + Freshness
- 배선 — 어디서 도는가

### 2.7.8 그라운딩 교환 포맷 — 서브 요청 → 메인 색인 → 반출 (신설 2026-08-22 · `plan_26082214` §4.2)

> 헌법 **불변식 B** 의 "어떻게". 헌법은 *왜 인용 없는 결정이 성립하지 않는가*를 말하고, 이 절은
> *그래서 무엇을 어떤 모양으로 주고받고 무엇이 기계 판정인가*를 정한다.
> 계약 = `sub_node/library-exchange.schema.json`(추적 PII-free 정적계약, 서브로 복제) ·
> 판정기 = `scripts/library_exchange.py`(**메인 전용** — 서브는 판정 주체가 아니다).
> 원리 차용 = `docs/report/ttsc-evidence-graph-principles-implementation.md` §1·§5.

**분업이 설계의 전부다 — *누락은 기계가, 거짓은 사람이.*** 서브가 "이 근거로 이렇게 빌드했다"고 말할
때, *그 근거가 실제로 반출된 항목인지·빠진 인용은 없는지*는 결정론으로 답할 수 있다. 반면 *그 근거가
이 결정을 정말 뒷받침하는가*는 사람이 읽어야 한다. 판정기는 앞엣것만 한다 — 뒤엣것을 흉내 내면 판정이
확률론이 되고 게이트가 무의미해진다.

**지식 비대칭은 유지한다.** 도서관(`__llm-wiki`)과 사서(`wiki-desk`)는 **메인 단독**이며 서브로
복제되지 않는다("DB 한 곳, 차등 접근권한"). 그래서 메인이 내보내는 것은 도서관이 아니라
**경로 + 앵커 + digest + 발췌**다 — 반출을 복제로 바꾸는 것이 비대칭을 깨는 실제 경로이므로,
발췌 예산 초과는 `EXPORT_EXCEEDS_EXCERPT_BUDGET` 로 거부한다.

#### 세 메시지 (전송은 신설하지 않는다 — 기존 A2A 통로에 실어 보낸다)

| # | `kind` | 방향 | 필수 블록 | 뜻 |
|---|---|---|---|---|
| ① | `library.citation.request` | 서브 → 메인 | `claim` · `query` | "이 **결정**을 뒷받침할 근거를 달라". 서브는 도서관을 뒤지지 않는다 — 무엇을 찾는지만 말한다 |
| ② | `library.resolution.export` | 메인 → 서브 | `resolution` · `references` | 사서가 색인한 결과 중 **반출 가능한 항목**. `status` ∈ resolved\|partial\|**unresolved**\|refused |
| ③ | `library.citation.attestation` | 서브 → 메인 | `claim` · `citations` · `decision` | "이 결정은 이 ref 들을 인용해 내렸다"는 자기귀속 |

- 셋은 `exchange_id` 로 묶인다. 갈리면 `EXCHANGE_ID_MISMATCH` — 어느 반출이 어느 결정을 뒷받침했는지
  추적 불가가 되므로 의미 판정을 **아예 진행하지 않는다**.
- **한 오브젝트는 한 종류만 담는다**(`KIND_BLOCK_FOREIGN`). 스키마의 `additionalProperties:false` 는
  "계약에 없는 키"만 막고 "이 종류에 없어야 할 블록"은 못 막으므로, 그 경계는 판정기가 닫는다.

#### 증거그래프 어휘 대응

| 증거그래프 | 여기서 | 내용 |
|---|---|---|
| **claim** (빚을 지는 쪽) | `claim` | 인용 의무를 지는 **결정**. `kind` ∈ build-decision · serve-decision · version-pin · patch-slot · *informational* |
| **reference** (빚의 원천) | `references[]` | 도서관 항목. `ref_id` · `path` · `anchor` · **`digest`** · `excerpt` · `authority` |
| **acknowledgement** | `citations[]` | claim → reference 의무 이행. `ref_id` · `digest` · `reason` |
| **resolution** | `resolution.status` | 사서의 해소 상태. **`unresolved` 는 정직한 공백**이지 실패가 아니다 |

- `authority` 는 wiki-desk 규약(**실행진실 > 계획의도**)을 그대로 쓴다 — 서브가 상충하는 참조를
  받았을 때의 우선순위다.
- `informational` 은 **인용 의무 모집단 밖**이다(질의만 하고 결정하지 않는 claim). 그것으로 결정을
  채택하면 의무를 우회하는 셈이라 `HOST_INELIGIBLE` 로 막는다.

#### 하드게이트 — 세 질문 + Freshness

```bash
python3 .claude/skills/terraforming_node/scripts/library_exchange.py \
        gate --request <req.json> --export <exp.json> --attestation <att.json>
# 0 = 통과 · 5 = 게이트 위반(fail-closed) · 2 = 사용오류
```

| 질문 | 무엇을 묻나 | 위반 코드 |
|---|---|---|
| **Resolution** | 모든 인용이 반출 항목 **정확히 하나**로 해소되는가 | `CITATION_UNRESOLVED` · `EXPORT_DUPLICATE_REF_ID` |
| **Host eligibility** | 인용을 단 결정이 **의무 모집단**인가 | `HOST_INELIGIBLE` |
| **Coverage** | 채택된 모든 결정이 **최소 하나**의 인용을 갖는가 | **`GROUNDING_OMISSION`** |
| *+ Freshness* | 인용 시점 digest 가 반출 시점 digest 와 같은가 | `CITATION_STALE` |

- **`GROUNDING_OMISSION` 이 불변식 B 의 집행점이다.** *"아무것도 인용하지 않는 산출물은 '필요했다'는
  증거가 없고, 반출되지 않은 것을 인용하는 산출물은 '무엇'도 증명하지 못한다."*
- **도서관에 근거가 없을 때의 올바른 행동은 강행이 아니다.** `resolution.status` 가
  `unresolved`/`refused` 인데 서브가 `decision.accepted: true` 를 내면 omission 이며, 판정기가
  그 경우 **`accepted:false` + HITL 에스컬레이션**이 정답임을 메시지에 함께 적는다.
- **Freshness 는 `requireReview` 차용**이다 — 인용된 것이 바뀌면 인용은 만료된다. 조용히 유효한 척하면
  낡은 근거 위에 선 결정이 초록불로 남는다.
- 판정기는 **서브 디스크를 읽지 않는다**(메시지만 본다) — §2.7.2 스캔 금지와 push-attestation 보존.

#### 배선 — 어디서 도는가

| 단계 | 주체 | 행위 |
|---|---|---|
| ① 요청 | 서브(A2A remote agent) | `library.citation.request` 를 task-report 통로로 반환 |
| ② 색인·반출 | **메인 + `wiki-desk`** | 사서가 색인 → 반출 가능분만 `library.resolution.export` 로 회신 |
| ③ 귀속 | 서브 | 결정과 함께 `library.citation.attestation` 반환 |
| ④ **판정** | **메인** | `library_exchange.py receive` — 통과해야 결정이 성립. 실패는 서브로 feedback |

- ✅ **자동 호출 배선 완료**(2026-08-22 · W-5). ④ 의 정문은 `gate` 가 아니라 **`receive`** 다 —
  `gate` 는 "이 셋이 정합한가"만 알고 *"지금 물어야 하는가"* 를 모른다. 그래서 2026-08-22 E2E 에서
  gate 를 돌린 주체는 파이프라인이 아니라 **사람**이었다(`testlog_26082215` §4.5).
  `receive` 는 리포트 자신에서 **인용 의무를 판정**하고(`phase ∈ {config,build,serve}` ∧
  `status=completed`), 의무가 있는데 교환 3메시지가 없으면 `GROUNDING_EXCHANGE_ABSENT` 로 거부한다.

```bash
# 위임과 수신을 한 명령으로 묶는다 — 두 단계로 두면 두 번째를 건너뛸 수 있다.
python3 .claude/skills/terraforming_node/scripts/library_exchange.py receive \
        --invoke-request <agent-control-request.json> --exchange-dir <교환 3메시지 디렉터리>
# 이미 받아 둔 산출로 판정할 때:
#   receive --agent-control-result <result.json> …   (result.output 에서 리포트를 꺼낸다)
#   receive --report <task-report.json> …
```

- **결속을 함께 본다**: `attestation.node_id` 가 리포트 `node_id` 와 다르면 `EXCHANGE_NODE_MISMATCH`
  로 거부한다 — 다른 노드/다른 결정의 통과한 교환을 재사용해 게이트를 우회하는 경로를 닫는다
  (M-2 가 노출한 `execution_approval` 재인가 결함과 **같은 형태**이며, 그쪽은 W-8 로 남아 있다).
- **유보에는 의무가 없다**: `input-required`·`failed` 는 채택이 아니므로 인용을 요구하지 않는다.
  요구하면 *"막혔다"고 정직하게 보고하는 경로가 오히려 벌을 받는다.*
- 전송은 형제 `scripts/agent_control.py`(provider-neutral orchestrator)가 소유한다 — `receive` 는 `agent_control.py invoke` 를 부르고
  **자기 전송을 만들지 않는다**. (07-25 "agent_control = 헌법 소유" 결정은 plan_26093022 에서 폐기 — 호출자가 이 스킬뿐이라 기초층 런타임이 아니다. Claude CLI 구문의 유일 발행처는 여전히 `providers/claude_code.py` 다.)
- 양 토폴로지 공통이다(§2.7.0 분기표) — 도서관 비대칭은 sub 가 Ray 워커든 A2A 에이전트든 같다.
