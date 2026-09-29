# trial-loop 실행 + 서빙 완료 마무리 (조건부 reference)

> spine step 8(trial-loop)·step 9(마무리)의 **루프 배선과 종결 시퀀스**. 수렴 판정·조정은 결정론
> (`sim_classify.py`/`recipe.py`), 이 문서는 그 루프의 운용 규칙과 종결 핸드오프다.

## 1. `recipe.py simulate` — 통합 trial-loop

```bash
# 배선/수렴 검증(docker 없이): mock-profile 의 vllm_profile+functional 로 결정론 테스트.
#   수렴 각인은 run_summary.json 에만 남는다(캠페인 밖 lockset 은 --lockset-out 없이는 덮어쓰지 않는다).
python3 recipe.py simulate --config config.yaml --candidate lockset.json \
  --dry-run --mock-profile mock.json --run-id 2026062122_1_qwen_sim --cap 3

# 실서빙(컨테이너 띄움): NAS 모델 존재 전제. 각인한 lockset 을 남길 자리를 --lockset-out 으로 준다.
python3 recipe.py simulate --config config.yaml --candidate lockset.json --lockset-out lockset.json \
  --image vllm-src-022:clean --topic qwen36-27b --cap 3

# 캠페인 셀: 자리를 파생하고(campaign_init.py --derive config|lockset --cell <id>) 셀 lockset 을 --candidate 로 주면
#   --lockset-out 을 생략해도 수렴 각인(provenance=explorer-phase2 · *_source · trial_provenance)이 그 파일에 쓰인다.
python3 recipe.py simulate --config "$(python3 campaign_init.py --derive config --cell <id>)" \
  --candidate "$(python3 campaign_init.py --derive lockset --cell <id>)" --topic <id> --cap 3
```

루프(cap 기본 3 = reconciliation_cap, workflow S3 와 동일):

```text
run_trial(candidate)            # docker run -d → /health 200 폴링 → functional_smoke → logs → parse_vllm_log → teardown
  → sim_classify(trial, budget, gate_margin[, typical_request_tokens])   # gate_margin=safety_margin (배포 gmu 아님 · plan_26091407 §4.3)
      none           → 측정 트라이얼이면: batch 산출(min(요구, KV-fit) · 요구 없으면 미정) → 클램프 산정(required ≤ 천장 budget × deploy_gmu) → 재검증 트라이얼
                       잠정 클램프(batch 산출 전 vram_oom 분) 트라이얼이면: 배포 천장 환산 KV-fit 으로 batch 산출 → 클램프 재산정 → 재검증
                       클램프 트라이얼 batch > 엔진 KV-fit → adjust_target=batch 로 낮춰 재검증(실패 아님 · 산식 재조정)
                       그 외 → 수렴. gen_recipe_set 3종 세트 + lockset 각인(--lockset-out · 캠페인 셀 lockset 이면 기본) + simlog write_summary + feedback(converged=True, 실측)
                       → 최종 serve-up 후 §2 마무리(lite 벤치 자동 핸드오프 · opt-out 경고 1줄 · 용처 팝업/유지·down)
      vram_oom       → 조정: kv_cache_memory_bytes = min(required, max_safe)  [실측 weights/overhead 기반]
      functional     → 조정: 실패한 soft 변수를 *_candidates 다음 후보로 폴백
      vram_infeasible→ HITL 즉시 중단(KV로 못 푸는 구조적 초과)
      unknown        → HITL 즉시 중단(알려진 시그니처 미매칭)
  → 반복 (cap 소진 시 HITL)
```

- **cap 의 lite ② 차감(2026-09-29 · `plan_26092923_58_27`)**: `--candidate` 가 캠페인 셀 lockset 이면 같은 셀 `cell.status.json` 의
  `reconciliation.charges[]`(lite ② `server_failed` 1건 = 1 · writer `campaign_init --cell-set --lite-raw`) 수만큼 cap 을 줄인다 —
  서빙은 됐지만 실사용 불가였던 재발동은 새 3회가 아니라 **남은 횟수**로 돈다. 잔여 ≤ 0 이면 트라이얼 없이 Model-C(exit 3).
  재발동 결정 기록(`--reentry-decide`)이 없으면 경고만 한다(승인 게이트는 대화 평면 · 차단 ✗). 셀 상태 판독 실패는 exit 5(차감 0 으로 접지 않는다).

- **준비 판정 = `:PORT/health` HTTP 200**. 로그의 "startup complete" grep 금지(거짓양성 — workflow S3 와 동일).
- **참조-그라운디드 해결 (`functional`·`unknown` 복구 — 자기추론 금지)**: `functional` 폴백 소진 또는 `unknown` 에 도달하면
  re-strategize/halt **전에** 권위 참조를 먼저 조회한다(여기서 토큰을 더 쓰는 것은 *권장*된다):
  ① **전체 `trialNN_vllm.log`**(루프는 regex excerpt 만 파싱 — 원문 전체로 실패 시그니처·oracle 경고 확인) →
  ② 모델 `config.json`/`chat_template`(파서·능력·아키 가정 검증) →
  ③ **빌드 이미지의 실제 vLLM 버전 + 레지스트리 정적 grep**(파서확증 기법을 *복구 루프에서도* 재실행) →
  ④ vLLM oracle 소스(예 `config/kernel.py` `MoEBackend`·선택 로직) →
  ⑤ **외부 교차검증(메인 한정 — 로컬 소스 소진 시 의무)**: `.claude/skills/wiki-desk/reference/references.md` §5 부정판정 최소범위 레시피를
  수행하고 **검색어·URL·일자를 testlog "탐색 증거" 섹션에 기록** — ⑤ 수행·기록 없이 "이 조합은 불가" 부정 보고 금지
  (egress-restricted 서브 = ⑤ 생략 + 증상 상향 · egress-online+위임 서브 = ⑤ 자율 수행). ①–⑤ 로도 미해소면 그제서야 Model-C HITL.
- **수렴 시**: `configs/<name>.yaml`(VRAM 분해 주석 + `max-num-seqs`·`kv-cache-memory-bytes`·`kv-cache-dtype`),
  `configs/<name>.sh`(`VLLM_ATTENTION_BACKEND` export + tool/reasoning 파서 플래그), `envs/.env.<name>` 생성.
- **미수렴(cap 소진/infeasible/unknown)** → **Model-C HITL** 보고: 증거 simlog 경로 제시, 비0 종료(3).
- **증거 발행**: `docs/simlog/<run_id>/` 에 `trialNN_vllm.log`·`_profile.json`·`_candidate.yaml`·`_smoke.json`,
  `correction_history.jsonl`, `run_summary.json`. 빌드/검증 판정은 `docs/testlog/` 에 별도 기록(`.claude/rules/docs.md`).
- **feedback_log**: invocation 당 1행, Phase 2 필드(`batch, kv_cache_memory_bytes, attention_backend, tool_call_parser,
  reasoning_parser, converged, trial_count, correction_history`)를 실측 채움.
- **teardown(필수)**: 트라이얼은 끝날 때마다 `docker rm -f` 로 컨테이너를 제거한다(통합메모리라 잔류 컨테이너가 다음 트라이얼을
  OOM 으로 떨어뜨림 — `run_trial` 의 `finally` 가 보장).

## 2. 서빙 완료 마무리 (plan_26071115 · Phase C)

Phase 2 수렴(`none`) + 최종 serve-up 성공 후, **최종 서빙유지 판정의 주체는 recipe-explorer**(런타임 스킬 중 서빙을
기동·유지·종료하는 최종 권위). 순서대로 수행한다.

- **① lite 벤치 자동 핸드오프**: 서빙 성공 직후 **adversarial-benchmark 의 lite 모드**(`lite_bench.sh`)로 핸드오프해 5종
  상태 스냅샷을 자동 수행한다. **lite 소유 = adversarial-benchmark** — recipe 는 *측정을 재구현하지 않고* 트리거만 한다.
  lite = inform-only · 기본 ON(가벼운 스킵 무시·강력 거부 구문만 세션 한정 억제). 헌법 §경량 벤치 자동 핸드오프 따름정리
  (명시 예외 — 관측·inform-only 한정). 괴리 의심 시 lite 는 **이상징후 안내 + full 승격 권유**까지(자동 loop-back ✗).
  사용자가 "커뮤니티에서 이 정도 나온다는데 검증해줘" 류 HITL 을 주면 그때 **full 벤치**로 승격.
  (2026-09-29 · `plan_26092923`) lite 는 이제 **성립 판정**을 낸다 — exit 6(① 측정 경로 불성립 · 하네스) / 7(② 서버 응답 실패 ·
  실사용 불가). 이 핸드오프(α)에서는 판정을 **기록·보고만** 한다: 서빙 성공 판정을 뒤집거나 재빌드·서빙전략 재수행을 자동으로
  시작하지 않는다(헌법 트리거 절). ② 는 사람에게 "실사용 불가 · 재발동 제안" 으로 올리고(stdout 제안 문구 · 캠페인 셀이면 cap 차감·제안은 셀 상태 writer 몫), ① 은 하네스 결함으로 올린다.
- **② opt-out 노드 서빙 경고 재확인**: 경고의 **발화 시점 정본은 terraforming §2.6 ③**(각 serve *기동 직전*). 이 마무리
  단계에서는 리스크를 **재고지**한다 — manifest `host_safety.installed` 를 읽어 **통합메모리 노드 ∧ `installed:false`** 면
  **에이전트 채팅 1줄**. **discrete 노드·installed:true 는 무경고 · serve 스크립트/로그 배너 코드변경 ✗**.
- **③ 서빙유지 / down 분기**:
  - **유지(기본)**: 서빙 완료 선언 → **용처 연결 매뉴얼 팝업**(④) → 서빙 유지.
  - **down(사용자 명시 "테스트만")**: serve → **lite 수행** → 결과 표시 → **down**. **용처 매뉴얼 생략**(라이브 엔드포인트 없음).
- **④ 용처 연결 매뉴얼 팝업**: 채팅 **info-only 팝업**(파일 산출물 없음). **살아있는 엔드포인트 실값**(host:port/v1·서빙 모델명) 반영.
  - **깊이 = 연결 방식만**: 대상 도구(OpenWebUI·Hermes Agent 등)의 **엔드포인트/API 필드 설정법** 중심. 대상 도구는
    **타 지역·타 서버 소재 가능** → **설치 가이드·설치 탐지 ✗**. 미설치로 보여도 공식 문서 링크 1줄만.
  - **외부검색 평면**: 획득모드와 무관하게 허용(서빙전략 외부 교차검증과 동일 평면). 검색 실패 시 **음성정직 + curl 기본 안내 폴백**.
    **egress-restricted 서브 = 메인 릴레이**.
  - **무답/"아몰랑"**: 외부검색 생략 — **OpenAI-호환 엔드포인트 curl 예시만**.
- **⑤ hint 태그 발동 신호 (셀 closer · 그 셀의 서빙·측정·문서 완료 후 · main-only)**: ①~④ 마무리가 끝나고 **그 셀의**
  서빙·측정(lite 이상)·문서(testlog·devlog 발행) 가 끝난 뒤, 이 셀의 파생 이름이 **새 태그**이면 hint 태그 발행을 **제안(Y/N)**
  한다. 캠페인 완주를 기다리지 않는다(셀 1개 = 태그 1개 · 2026-09-08 "발행을 캠페인 완주와 분리" · plan_26092119 D9).
  이름은 사람도 recipe 도 짓지 않는다 — 발행기가 전량 파생하고, 미리보기는 읽기 전용
  `hint.py name --campaign <id> --cell <cell>` 이다(이미 원격에 있는 이름은 publish 가 `HINT_NAME_COLLISION` 으로 막는다).
  **recipe 는 트리거·신호만** — 발행은 hint-publisher 단일 진입 `.claude/skills/hint-publisher/scripts/hint.py` 가 소유한다:
  `hint.py publish --campaign <id> --cell <cell> --generated-utc <UTC>`(증거·발행 자격 관측·이름·계보·산출물 → 스캐폴드 후 **정지**) → Agent 가 PROMPT
  절의 서사·hint-event·발췌를 저작 → `hint.py continue --campaign <id> --cell <cell> --generated-utc <UTC>`(**승인 확인이 먼저**
  — 없으면 브랜치·태그 부수효과 0 인 채 `HINT_APPROVAL_ABSENT` 로 정지 → 린트 → hint 브랜치 배관 커밋 → 봉인·로컬 검증 →
  그 태그 1개 push → 원격 SHA 대조 → 카탈로그 재파생). 승인은 캠페인 셀이면 선언 확인 팝업에서 받은 `hint_targets[].approval`
  (`campaign_init.py --hint-approve`)이고, 그 밖은 `continue --approved-by "<사람 발화 전사>" --approved-utc <UTC>` 다.
  셀 문서(testlog·devlog)는 `campaign_init.py --evidence-add` 로 그 셀의 포인터에 등록돼 있어야 publish 가 발행 기록을 만든다
  (없으면 `HINT_NARRATIVE_EVIDENCE_ABSENT`).
  push 는 `refs/tags/hint/<그 태그>` 하나뿐이며 hint 브랜치는 밀지 않는다(무인 자동 태깅 ✗). 캠페인 밖 입력 형태
  (`--publication <topic> --replay`)와 절차 정본은 hint-publisher SKILL.md 다.
  egress-restricted 서브 = 발행 ✗(main-only) · 증상만 상향.
