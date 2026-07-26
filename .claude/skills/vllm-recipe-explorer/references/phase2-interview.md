# Phase 2 인터뷰 — lock/soft/free 확정 + 서빙 UX 4항목 (조건부 reference)

> spine step 7(인터뷰 → lock-set)의 **질문지·함정·산출 스키마**. 실서빙 trial-loop 전에 LLM 이 사람과
> 멀티턴으로 변수를 확정한다. **타깃 GPU 가 0순위**(예산이 모든 게이트의 기준). 사람이 모르면 결정론 산출값을 제안한다.

## 1. 기술 변수 인터뷰 (순서대로)

1. **타깃 GPU / VRAM 예산** (필수, 0순위 · 폴백 명문화 D10) — *"따로 시뮬레이션 타겟 GPU 가 있나요?"* 물어 분기한다:
   **스킵/"호스트" 답변 → 호스트 GPU 기준**(manifest HW사실) · **명시 타겟 → `config.target_gpu`**(γ 시뮬레이터 모드,
   **VRAM 수준 한정** 시뮬레이션). 예: RTX PRO 6000 96GB. → `config.yaml` 의 `vram_budget_gb`(스칼라, 하위호환) 또는
   host≠target 이식이면 구조화 `target_gpu` 블록(`kv-clamp.md` §타겟-GPU 이식형 예산).
   **이전 전제**: host 에서 측정한 attention backend 와 타겟이 동일해야 KV 레이아웃·overhead 가 유효 이전된다 —
   트리플렛 헤더에 명시 pin(`gen_recipe_set.py` 가 이미 `attention_backend` 를 emit).
2. **weight quant?** — prequantized 면 native 고정. 비prequantized 면 `none`/`fp8`(awq/gptq 는 비prequantized 경고).
3. **KV quant?** — `null`(fp16, 2바이트) 또는 `fp8`(1바이트). KV 캐시를 절반으로 줄여 더 긴 context/batch 확보.
4. **batch**(=동시요청수, `--max-num-seqs`) → 정한 뒤 **max-model-len** — *최대 가능값을 제안*한다
   (`estimate_vram.max_feasible_max_len` 이 천장 내 2의 거듭제곱 최대 길이를 결정론으로 계산).
5. **tool / reasoning 파서** — **외부 교차검증**(HF 모델카드·docs, *에이전트 수행*): 모델이 tool_call·reasoning 을 지원하는지,
   vLLM 파서명이 무엇인지 에이전트가 확인한다(예: `hermes`/`qwen3`). 미지원이면 N/A(스모크에서 스킵).
   - **파서명은 공식 docs 에서 얻은 뒤 반드시 빌드 이미지에 version-exact 확증한 후에만 emit 한다(가정 금지)**:
     레지스트리 경로·등록명이 vLLM 버전마다 다르다 — reasoning 은 `vllm/reasoning/`, tool 은 버전에 따라
     `vllm/entrypoints/openai/tool_parsers/` 또는 `vllm/tool_parsers/`. **정적 grep + 실서빙 수용**으로 확증한다
     (import-기반 레지스트리 열거는 lazy-registration 이라 0.18.0 에서 거짓-빈값 → 비의존). 예: gemma-4 reasoning/tool=`gemma4`;
     gpt-oss reasoning=`openai_gptoss`(0.18.0 실서빙 실증).
   - **caveat — gpt-oss tool 처리는 빌드별 상이**: 0.18.0 은 tool 호출이 **vLLM harmony 내장**이라 `--tool-call-parser` 가
     불요/미등록이고 vLLM 이 `--enable-auto-tool-choice` 를 무시하고 항상 tool use 를 켠다 → 무효 파서 플래그를 주면 serve 크래시.
     tool capability=false 로 두고 **completion+reasoning 을 스모크 게이트**로.
   - **caveat — reasoning 분리필드명 버전차 + 추론모델 max_tokens**: 응답의 분리 reasoning 필드명이 버전마다 다르다
     (구=`reasoning_content`, 0.18.0/harmony=`reasoning`) → `functional_smoke` 는 둘 다 수용. **추론모델은 completion
     스모크에도 max_tokens 충분히** 줘야 한다(analysis 채널이 토큰 소진 → content 전 length 절단; 64→1024).
   - **caveat — tool 채팅 템플릿 의존성**: gemma-4 tool 은 `tool_chat_template_gemma4.jinja`(이미지 미포함)를 요구한다 →
     `.sh` 에 tool 플래그를 무조건 emit 하면 런타임 실패. reasoning 이 기본 OFF 면 **completion 이 스모크 게이트**이고
     파서는 config 에 기록만 한다(`gen_recipe_set` 의 경고 가드).
6. **attention backend**(soft) — `FLASHINFER`/`FLASH_ATTN`/`FLASHMLA`. 폴백 후보 순서를 정한다.

## 2. 변수 3계층

| 계층 | 의미 | 키 |
|------|------|----|
| **lock** | 인터뷰로 고정, 루프가 안 바꿈 | `quantization, max_model_len, batch, kv_cache_quant` |
| **soft** | 실패 시 `*_candidates` 순서로 폴백 | `attention_backend, tool_call_parser, reasoning_parser` |
| **free** | 루프가 결정론으로 산정 | `kv_cache_memory_bytes` |

인터뷰 산물 = **lock-set JSON**(`lockset.json`). soft 변수마다 순서있는 `*_candidates` 폴백 리스트와
`model_capabilities: {tool_call, reasoning}` 를 채워 `recipe.py simulate` 에 투입한다.

```json
{ "id": "s1", "quantization": "none", "max_model_len": 32768, "batch": 8,
  "kv_cache_quant": null, "kv_cache_memory_bytes": null,
  "attention_backend": "FLASHINFER", "attention_backend_candidates": ["FLASHINFER", "FLASH_ATTN"],
  "tool_call_parser": null, "tool_call_parser_candidates": [],
  "reasoning_parser": null, "reasoning_parser_candidates": [],
  "model_capabilities": { "tool_call": false, "reasoning": false } }
```

## 3. 서빙 UX 인터뷰 4항목 (Convenience 보강 · plan_26071115 · Phase C)

기술변수(lock/soft/free) 확정과 **별개로** 서빙 경험을 매끄럽게 하는 **4항목**을 함께 수집한다.
**이 4항목은 lock/soft/free 변수가 아니다** — lockset 표·`lockset.json`·`run_trial` serve-args 에 넣지 않는다.

| 항목 | 질문 요지 | 기본값(무답) | 분기 |
|------|-----------|--------------|------|
| ① 타겟 GPU | (§1 item 1 재참조) "따로 시뮬 타겟 GPU 있나요?" | **호스트 GPU** | 명시 타겟 → γ 시뮬레이터 모드(VRAM 한정) |
| ② 벤치마크 | **질문 아님** — lite 기본 ON 고지 | lite 자동 수행 | 강력 거부 구문만 → **세션 한정** 억제 |
| ③ 용처 | "서빙되면 어디에 연결해 쓰실 건가요? (예: OpenWebUI·Hermes Agent·직접 API…)" | 무답/"아몰랑" → curl 예시만 | 열린 목록 — 어떤 답이든 외부검색 시도 |
| ④ 컨테이너 | "서빙 후 유지할까요, 테스트만 하고 내릴까요?" | **유지** | down → `serving-closeout.md` 시퀀스 |

- ①·③·④ 는 **서빙 *전* 사전 수집**(D11) → ③ 용처 매뉴얼은 **서빙 *후*** 팝업(2-phase). ② 는 질문이 아니라 **고지**
  (lite 는 adversarial-benchmark 소유·기본 ON).
- **컨테이너 기본 = 유지**: 서빙 성공 후 컨테이너를 살려 둔다(per-trial teardown 은 trial-loop 통합메모리 OOM 보호용으로
  최종 serve 유지와 **별개 평면**). down 은 사용자 명시 시에만, **down 전 lite 필수**.
