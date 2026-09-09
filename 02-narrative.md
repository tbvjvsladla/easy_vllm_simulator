# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26090916_hf_overrides_SERVE_KNOB_추가_및_셀D_YaRN524k_재개.md`
- testlog: `../testlog/testlog_26090916_Qwen3.8.27B_광의탐색_셀D_YaRN524k_FullBench.md`

## 서사

이 태그는 자매 hint `hint/0.28.0/qwen3.8-27b/rtxpro6000-main-native/qfp8-len262144-kvfp8`
(셀 B)와 **동일한 환경·동일한 벽**을 겪었다(같은 서빙 세션, 같은 광의탐색 캠페인의 다른 셀) —
Docker 부재·torchvision 누락·TP 오염·tool_call_parser 오독 방지는 그쪽 태그 본문(§1·§3)을
참조하라. 이 셀 고유의 차이점만 아래에 적는다.

**증상(이 셀 고유)**: 이 캠페인이 애초에 4셀(non-YaRN 3 + YaRN 1)을 목표했으나, 이 셀(YaRN
524288)만 착수 시 구조적 불가로 보류됐다 — 원인은
`docs/devlog/devlog_26090916_hf_overrides_SERVE_KNOB_추가_및_셀D_YaRN524k_재개.md` §무엇을
했나 참조: 이 프로젝트의 레시피 생성기(`gen_recipe_set.py`)가 `--hf-overrides`(YaRN 확장에
필요한 vLLM CLI 플래그)를 SERVE_KNOB_KEYS 닫힌열거 밖에 두고 있어 트리플렛까지 전달할 통로가
없었다. **원인**: 그 노브가 애초에 이 생성기 설계에 없었을 뿐, vLLM 자체는 `--hf-overrides` 를
표준 CLI 플래그로 지원한다(공식 YaRN 확장 가이드가 존재 — 같은 devlog §핵심 결정 참조). **해소**:
`gen_recipe_set.py`·`run_trial.py` 에 `hf_overrides` SERVE_KNOB 을 추가(candidate 가 이미
완성한 JSON 문자열을 재해석 없이 옮기고, vLLM `--config` YAML 로더 계약에 맞춰 겹인코딩으로
emit — 같은 devlog §핵심 결정 참조) → 파리티 확인 → Phase 2 재수렴 → Full Bench PASS.

## 되풀이하지 말 것

1. Docker/torchvision/TP/tool_parser 벽은 자매 hint(B) 본문 §1·§3 참조 — **동일하게 적용된다**.
2. **YaRN 확장이 필요한데 이 레시피 생성기(gen_recipe_set.py 계열)가 그 CLI 플래그를 모른다고
   "런타임 패치가 필요하다"고 성급히 결론짓지 마라.** 생성기의 `void_reason` 문구를 문자 그대로
   믿었으면 잘못된 방향(`<model>_patch.py` 슬롯 신설)으로 갔을 것이다 — 실제로는 vLLM CLI 를
   직접 소스에서 확인해 표준 플래그(`--hf-overrides`)임을 확인하고, 트리플렛 SERVE_KNOB 확장으로
   해결했다(3+1+1 슬롯 판정 기준 = "무엇을 고치나"가 아니라 "언제 성립해야 하나").
3. **YaRN rope_parameters 값을 추측하지 마라.** 모델의 실제 `config.json#text_config.
   rope_parameters` 를 직접 읽어 그 값을 그대로 쓰고 `rope_type`·`factor` 만 목표 컨텍스트에
   맞춰 오버라이드했다(추측 시 정확도 저하 위험).
