# trial-loop 실행 + 서빙 완료 마무리 (조건부 reference)

> spine step 8(trial-loop)·step 9(마무리)의 **루프 배선과 종결 시퀀스**. 수렴 판정·조정은 결정론
> (`sim_classify.py`/`recipe.py`), 이 문서는 그 루프의 운용 규칙과 종결 핸드오프다.

## 1. `recipe.py simulate` — 통합 trial-loop

```bash
# 배선/수렴 검증(docker 없이): mock-profile 의 vllm_profile+functional 로 결정론 테스트.
python3 recipe.py simulate --config config.yaml --candidate lockset.json \
  --dry-run --mock-profile mock.json --run-id 2026062122_1_qwen_sim --cap 3

# 실서빙(컨테이너 띄움): NAS 모델 존재 전제.
python3 recipe.py simulate --config config.yaml --candidate lockset.json \
  --image vllm-src-022:clean --topic qwen36-27b --cap 3
```

루프(cap 기본 3 = reconciliation_cap, workflow S3 와 동일):

```text
run_trial(candidate)            # docker run -d → /health 200 폴링 → functional_smoke → logs → parse_vllm_log → teardown
  → sim_classify(trial, budget, margin)
      none           → 수렴. gen_recipe_set 3종 세트 + simlog write_summary + feedback(converged=True, 실측)
                       → 최종 serve-up 후 §2 마무리(lite 벤치 자동 핸드오프 · opt-out 경고 1줄 · 용처 팝업/유지·down)
      vram_oom       → 조정: kv_cache_memory_bytes = min(required, max_safe)  [실측 weights/overhead 기반]
      functional     → 조정: 실패한 soft 변수를 *_candidates 다음 후보로 폴백
      vram_infeasible→ HITL 즉시 중단(KV로 못 푸는 구조적 초과)
      unknown        → HITL 즉시 중단(알려진 시그니처 미매칭)
  → 반복 (cap 소진 시 HITL)
```

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
- **⑤ hint 태그 발동 신호 (신규모델 closer · 전작업 완료 후 · main-only)**: ①~④ 마무리가 끝나고 workflow S4(커밋·문서·전파)까지
  종결된 **최후**에, 이 서빙이 **새 `(vllm×model×arch)` 조합**이면 hint 태그 발행을 **제안(Y/N)** 한다. **recipe 는 트리거·신호만** —
  엔진(`.claude/skills/upstream-version-watch/scripts/hint_tag.py`)·발행 소유·push 는 workflow S4/헌법 §hint 배포 레이어 따름정리(무인 자동 태깅 ✗ · 모든 push=사용자 소관).
  egress-restricted 서브 = 발행 ✗(main-only) · 증상만 상향.
