---
name: vllm-recipe-explorer
description: >-
  폐쇄망에서 고정된 한 모델의 config.json을 결정론적으로 파싱하고, (quantization × max-model-len ×
  gpu-memory-utilization) 3축 레시피 후보를 결정론 VRAM 추정 → 예산×안전마진 하드게이트 →
  headroom→context 랭킹으로 제시하고, 사람이 고른 레시피를 DGX Spark 서빙용 3종 세트(.yaml+.sh+.env)로
  생성한다(Phase 1, 추정). Phase 2는 멀티턴 인터뷰로 lock/soft/free 변수를 정하고 실서빙 trial-loop로
  절대 KV 클램프(--kv-cache-memory-bytes)를 수렴시켜 검증된 레시피를 낸다. "레시피 추천",
  "서빙 레시피", "VRAM 예산에 맞춰", "이 모델 어떤 설정으로 띄울까", "RTX4090 예산 레시피",
  "gpu memory 예산", "recipe", "실제로 띄워서 검증", "KV 캐시 튜닝", "batch/동시요청", "tool/reasoning 파서",
  "attention backend" 같은 지시에 발동. tp는 git 브랜치 자동결정.
---

# vllm-recipe-explorer

고정된 **한 모델**을 여러 서빙 레시피로 비교해 **타깃 GPU 예산**(예: RTX PRO 6000=96GB,
RTX4090=24GB carve-out)에 맞는 설정을 찾는다. 두 페이즈로 동작한다.

- **Phase 1 (추정·추천)**: `config.json` 결정론 파싱 → LLM 후보 생성(탐색) → 결정론 VRAM 추정·하드게이트·
  랭킹 → 리포트 → HITL 선택 → 3종 세트 생성 + 되먹임 로그. *실서빙 없음(추정만).*
- **Phase 2 (실서빙 검증)**: 멀티턴 인터뷰로 lock/soft/free 변수 lock-set 확정 → **실서빙 trial-loop**
  (run_trial→sim_classify→조정→반복, cap 기본 3)로 **절대 KV 클램프**를 실측 수렴 → 수렴 시 동일 3종 세트
  생성 + simlog 증거 발행. *실제 컨테이너를 띄워 `/health` + 기능 스모크로 검증한다.*

> 설계 원칙(하네스 엔지니어링): **결정/게이트/랭킹/분류는 결정론 스크립트**(`scripts/`)가 책임진다.
> LLM은 후보 '생성'(탐색)과 인터뷰만 한다. OOM-critical 경로(VRAM 추정·게이트·OOM 분류)를 확률론으로
> 단정하지 않는다. whichllm 패키지를 import 하지 않는다 — 값/공식은 소스에서 **복사(벤더링)**하고 출처 주석을 단다.

근거: `docs/plan/plan_2026060819_1_whichllm_vLLM레시피탐색스킬_설계계획.md`(Phase 1 승인본),
`docs/plan/plan_2026062121_1_..._VRAM시뮬레이터_설계계획.md`(Phase 2), Seed `seed_e02364cce49d`.

## 0. 입력 (config.yaml)

사람이 미리 작성한 `config.yaml`(영속)을 읽는다. 없으면 `config.example.yaml`을 참고해 요청한다.

- `target_model.path`: 대상 모델의 **컨테이너 경로**(`/app/models/<Org>/<Name>`, `configs/*.yaml`의 `model:`과 동일).
- `nas_host_root`: `/app/models`가 매핑되는 호스트 NAS 루트(기본 `/mnt/models` — 실제 값은 `config.yaml`).
- `vram_budget_gb`: carve-out 가상 GPU 예산(예: RTX4090=24).
- `safety_margin`: 하드게이트 임계(기본 0.90).
- `kv_cache_dtype_bytes`: KV dtype 바이트(기본 2=fp16).
- `test_device_total_gib`(Phase 2): 측정 하드웨어 total VRAM(GB10=121.69, torch.cuda 기준). consolidated
  메모리 라인이 없는 vLLM 빌드에서 overhead 유도(`gmu×total−weights−kv`)에 쓴다. DGX Spark는 nvidia-smi가 N/A.
- `tensor_parallel_size`(선택): 미지정 시 git 브랜치 자동(`single-node`→1, `multi-node`→2, 기타→1).
- `serving.{config_name, port, served_model_name}`: 생성될 3종 세트의 base 이름·포트·서빙명.

**하드 제약**: 대상 모델이 NAS 경로에 없으면(디렉토리/`config.json` 부재) **다운로드하지 말고 비0 종료 + 중단·보고**.

**NAS 마운트 불변식(포인터 원칙, 헌법 §배포·환경 교차참조)**: 컨테이너 NAS 마운트 경로를 스크립트에
하드코딩하지 않는다 — 호스트 NAS 루트는 `config.yaml`의 `nas_host_root`에서 읽어 `/app/models`에 매핑한다
(`run_trial`은 이미 이 값으로 파라미터화됨 — 정정 대상이 아니라 유지해야 할 불변식). 컨테이너 경로
(`/app/models/...`) ↔ 호스트 경로 변환은 이 한 포인터로만 이뤄진다.

## 1. 결정론 / 확률론 경계 (하네스 엔지니어링)

| 구성요소 | 방식 | 비고 |
|----------|------|------|
| config.json 파싱 | **결정론 스크립트** | `parse_model_config.py` — text_config 중첩·safetensors 헤더 실측 |
| 후보 **생성**(brainstorm) | **LLM (이 단계만 확률론)** | 3축 조합 다양성이 가치(Generate&Filter의 Generator) |
| VRAM 추정 | **결정론 스크립트** | `estimate_vram.py` — plan §8 공식 리터럴 준수 |
| 하드 안전 게이트(margin) | **결정론** | 예산×margin 초과 후보 탈락(zero tolerance) |
| Judge 랭킹(headroom→context) | **결정론** 정렬 | 품질·속도 인자 없음 |
| 3종 세트 생성 | **결정론** 템플릿 | 기존 워크스페이스 스키마 준수 |
| **인터뷰**(타깃 GPU·quant·batch·파서…) | **LLM (확률론)** | Phase 2. soft 변수에 `*_candidates` 폴백 리스트를 채움 |
| **VRAM 로그 파싱**(실측 분해) | **결정론** | `parse_vllm_log.py` — 정규식, 라인 부재 시 null |
| **트라이얼 분류**(OOM/infeasible/functional) | **결정론** | `sim_classify.py` — 알려진 OOM 정규식 + 천장 점검 |
| **KV 클램프 재산정**(min(required,safe)) | **결정론** | `recipe.py simulate` 루프 — 실측 weights/overhead 기반 |

→ "**결정의 책임은 결정론 계층이 진다. 탐색·인터뷰만 LLM.**"

## 2. 파이프라인

`recipe.py`(오케스트레이터)가 함수 import로 조립한다. tp는 git 브랜치 자동(또는 config 우선).

### ① parse (결정론)

```bash
python3 scripts/parse_model_config.py <path> [--nas-root R] [--json] > parsed.json
```

- `/app/models/...` 입력은 `nas_host_root`로 치환해 호스트 NAS에서 읽는다(컨테이너 경로 ↔ 호스트 경로 매핑).
- **text_config 중첩 처리**: `text_config`가 있으면 아키텍처 필드를 그쪽 우선·top-level fallback으로 읽는다
  (`vision_config`는 무시). 예: Qwen3.5-35B-A3B는 전 필드가 `text_config` 아래.
- **num_params 산출(폐쇄망 로컬, 결정론)**: 비prequantized 모델은 **index가 가리키는 정본 샤드들의
  safetensors 헤더에서 텐서별 numel(shape 곱)을 합산** — 저장 dtype과 무관한 정확한 파라미터 수다
  (config `torch_dtype`이 거짓이거나[Motif-2.6B: 디스크 fp32지만 bf16 모델], 한 디렉토리에 복수 정밀도
  체크포인트가 공존해도[LFM2-8B-A1B: BF16+F32 두 벌] 정확). prequantized(mxfp4 등)는 `quant_method_native`
  bpw로 native weight를 나눈 best-effort(quant 축은 native 고정). `native_weight_bytes`는 index
  `total_size`(없으면 파일 크기 합), `disk_bpw`는 최대 safetensors 헤더의 온디스크 dtype 실측(폴백·보고용).
- 디렉토리/`config.json` 부재 → 명시적 에러(비0 종료, 다운로드 금지).

### ② LLM 후보 생성 (이 단계만 확률론)

모델 특성(`is_moe`, `num_params`, `max_position_embeddings`, prequantized 여부 등)을 고려해 3축 조합을
brainstorm 한다.

- `quantization`: prequantized면 **native 고정**(`quant_method_native`). 비prequantized면 `{none, fp8}`
  (오프라인 사전양자화 전제로 `awq`/`gptq`를 제안할 땐 **경고 표기** — 폐쇄망에서 런타임 양자화 불가).
- `max-model-len`: **4096부터 `max_position_embeddings`까지 2의 거듭제곱**(§9.1 규칙). `max_position` 미상이면
  `[4096, 8192, 16384, 32768]`.
- `gpu-memory-utilization`: 0.80~0.95(기본 그리드 `[0.85, 0.90, 0.95]`).

생성 후보를 `candidates.json`으로 저장 → `recipe.py estimate --candidates candidates.json`로 투입한다.
LLM 후보 없이 결정론 기본 그리드만 쓰려면 `--auto`(데카르트 곱)를 사용한다.

### ③ rank (결정론 하드게이트 + Judge) → 리포트

```bash
python3 recipe.py estimate --config config.yaml --auto
# 또는: python3 recipe.py estimate --config config.yaml --candidates candidates.json
```

- 각 후보 → `estimate_vram.py`로 **`(weight_bytes/tp + KV_cache + overhead) / gpu_memory_utilization`** 추정
  (plan §8 공식 리터럴). KV·overhead는 `/tp` 하지 않음 = 멀티노드 보수적 과대추정 → OOM 게이트에 안전.
- **하드게이트**: `estimated_total_gib ≤ vram_budget_gb × safety_margin` 초과 후보 전부 탈락(zero tolerance).
- **Judge**: 통과 후보를 `headroom_gib` 내림차 → `max_model_len` 내림차로 정렬(id 보존).
- 리포트 표 컬럼: `id, quant, max_len, gmu, est_vram_gib, headroom_gib, PASS/FAIL` + 헤더(모델 id·num_params·
  budget·margin·tp·경고). ranked 결과는 `feedback/.last_ranking.json`에 저장(generate가 id로 참조).

### ③.5 HITL — 사람이 rN 선택

리포트를 사람이 보고 통과 후보 중 하나의 `recipe_id`(예: `r3`)를 고른다. 무feasible(전부 FAIL)이면
예산 상향/마진 완화/모델 변경을 보고한다(자동 강행 금지).

### ④ generate (결정론 3종 세트) + 되먹임 로그

```bash
python3 recipe.py generate --config config.yaml --recipe-id r3
```

- `.last_ranking.json`에서 `r3`을 찾아 `gen_recipe_set.py`로 **3종 세트**를 생성(기존 워크스페이스 스키마 준수):
  - `configs/<name>.yaml` — `model: <container_path>`, `host 0.0.0.0`, `port 8000`,
    `gpu-memory-utilization`/`max-model-len`, `quantization`은 **native/none이 아닐 때만** 추가.
  - `configs/<name>.sh` — 기존 `gpt-oss-20b-normal.sh` 구조(TIKTOKEN 가드 + `vllm serve --config ... --served-model-name`).
  - `envs/.env.<name>` — `COMPOSE_PROJECT_NAME·CONTAINER_NAME·VERSION·NVIDIA_VISIBLE_DEVICES·SERVING_IP/PORT·
    TIKTOKEN_ENABLED·SERVING_MODEL_NAME·CONFIG_FILE`.
  - 기존 파일이 있으면 `--force` 없이는 덮어쓰지 말고 에러.
- `feedback_log.py`가 invocation당 1행(JSONL)으로 append한다(추정 필드 채움 + 실측 nullable 예약).
  생성된 `configs/<name>.yaml`은 수정 없이 `vllm serve --config`로 사용 가능하다.

## 3. 안전 (절대 규칙)

- **모델 자체 다운로드 금지.** NAS 경로에 없으면 비0 종료 + 명확한 중단·보고. 런타임 다운로드 없음.
- 호스트 `python3`(3.12, PyYAML 6) 단독 실행. **whichllm 패키지를 import 하지 말 것**(값은 벤더링).
- stdlib + yaml만 사용. **외부 네트워크 호출 금지**(폐쇄망 전제 — 인터뷰의 웹검색은 사람이 수행해 결과를 주입).
- 결정/게이트/랭킹/분류는 결정론 스크립트가 책임진다(LLM은 후보 탐색·인터뷰만).
- **Phase 2 teardown(필수)**: 트라이얼은 끝날 때마다 `docker rm -f`로 컨테이너를 제거한다.
  DGX Spark는 **통합메모리**라 잔류 컨테이너가 다음 트라이얼을 OOM으로 떨어뜨린다(`run_trial`의 `finally`가 보장).
- 빌딩블럭(`.claude/`)은 gitignored 유지 — 외부/공식 스킬·플러그인은 propose→review→install.

## 4. Phase 2 — 멀티턴 인터뷰 → lock-set

실서빙 trial-loop 전에 LLM이 사람과 **멀티턴 인터뷰**로 변수를 확정한다. **타깃 GPU가 0순위**(예산이
모든 게이트의 기준). 순서대로 묻되, 사람이 모르면 결정론 산출값을 제안한다.

1. **타깃 GPU / VRAM 예산** (필수, 0순위) — 예: RTX PRO 6000 96GB. → `config.yaml`의 `vram_budget_gb`.
2. **weight quant?** — prequantized면 native 고정. 비prequantized면 `none`/`fp8`(awq/gptq는 폐쇄망 경고).
3. **KV quant?** — `null`(fp16, 2바이트) 또는 `fp8`(1바이트). KV 캐시를 절반으로 줄여 더 긴 context/batch 확보.
4. **batch**(=동시요청수, `--max-num-seqs`) → 정한 뒤 **max-model-len** — *최대 가능값을 제안*한다
   (`estimate_vram.max_feasible_max_len`이 천장 내 2의 거듭제곱 최대 길이를 결정론으로 계산).
5. **tool / reasoning 파서** — **웹검색 권장**: 모델이 tool_call·reasoning을 지원하는지, vLLM 파서명이 무엇인지
   사람이 확인해 알려준다(예: `hermes`/`qwen3`). 미지원이면 N/A(스모크에서 스킵).
   - **파서명은 공식 docs에서 얻은 뒤 반드시 빌드 이미지에 version-exact 확증한 후에만 emit한다(가정 금지)**:
     레지스트리 경로·등록명이 vLLM 버전마다 다르다 — reasoning은 `vllm/reasoning/`, tool은 버전에 따라
     `vllm/entrypoints/openai/tool_parsers/` 또는 `vllm/tool_parsers/`. **정적 grep + 실서빙 수용**으로 확증한다
     (import-기반 레지스트리 열거는 lazy-registration이라 0.18.0에서 거짓-빈값 → 비의존). 예: gemma-4 reasoning/tool=`gemma4`;
     gpt-oss reasoning=`openai_gptoss`(0.18.0 실서빙 실증).
   - **caveat — gpt-oss tool 처리는 빌드별 상이**: 0.18.0 은 tool 호출이 **vLLM harmony 내장**이라 `--tool-call-parser`가
     불요/미등록(tool 레지스트리=`kimi_k2` only)이고 vLLM이 `--enable-auto-tool-choice`를 무시하고 항상 tool use를 켠다
     (서빙 로그 명시) → 무효 파서 플래그를 주면 serve 크래시. tool capability=false로 두고 **completion+reasoning을 스모크
     게이트**로(다른 빌드의 `openai` tool 파서 경로와 구분 — 대상 이미지서 확증).
   - **caveat — reasoning 분리필드명 버전차 + 추론모델 max_tokens**: 응답의 분리 reasoning 필드명이 버전마다 다르다
     (구=`reasoning_content`, 0.18.0/harmony=`reasoning`) → `functional_smoke`는 둘 다 수용. **추론모델은 completion
     스모크에도 max_tokens 충분히** 줘야 한다(analysis 채널이 토큰 소진 → content 전 length 절단; `functional_smoke` 64→1024).
   - **caveat — tool 채팅 템플릿 의존성**: gemma-4 tool은 `tool_chat_template_gemma4.jinja`(이미지 미포함)를
     요구한다 → `.sh`에 tool 플래그를 무조건 emit하면 런타임 실패. reasoning이 기본 OFF면 **completion이 스모크
     게이트**이고 파서는 config에 기록만 한다(`gen_recipe_set`의 경고 가드로 처리).
6. **attention backend**(soft) — `FLASHINFER`/`FLASH_ATTN`/`FLASHMLA`. 폴백 후보 순서를 정한다.

**변수 3계층**:

| 계층 | 의미 | 키 |
|------|------|----|
| **lock** | 인터뷰로 고정, 루프가 안 바꿈 | `quantization, max_model_len, batch, kv_cache_quant` |
| **soft** | 실패 시 `*_candidates` 순서로 폴백 | `attention_backend, tool_call_parser, reasoning_parser` |
| **free** | 루프가 결정론으로 산정 | `kv_cache_memory_bytes` |

인터뷰 산물 = **lock-set JSON**(`lockset.json`). soft 변수마다 순서있는 `*_candidates` 폴백 리스트와
`model_capabilities: {tool_call, reasoning}`(웹검색 산물)을 채워 `recipe.py simulate`에 투입한다.

```json
{ "id": "s1", "quantization": "none", "max_model_len": 32768, "batch": 8,
  "kv_cache_quant": null, "kv_cache_memory_bytes": null,
  "attention_backend": "FLASHINFER", "attention_backend_candidates": ["FLASHINFER", "FLASH_ATTN"],
  "tool_call_parser": null, "tool_call_parser_candidates": [],
  "reasoning_parser": null, "reasoning_parser_candidates": [],
  "model_capabilities": { "tool_call": false, "reasoning": false } }
```

## 5. Phase 2 — 절대 KV 클램프 (가장 중요)

vLLM 인자 **`--kv-cache-memory-bytes`**(GPU당 KV 바이트, 정수)로 KV 캐시를 **절대값으로 고정**한다.
이 인자를 주면 **`gpu-memory-utilization`은 무시**된다(분율이 아니라 절대 바이트가 제어).

```text
Phase 2 총 VRAM = weights + non_kv_overhead + kv_cache_memory_bytes     ← gmu로 나누지 않음
검증 게이트     = (weights + overhead + kv_bytes) ≤ budget_gib × safety_margin × GiB
```

- **Phase 1 공식과 다름**: Phase 1은 `(…)/gmu`로 나눴다. Phase 2는 절대값 합이다. 혼동 금지.
- **`gpu-memory-utilization`은 반드시 명시적으로 emit한다**(절대 기본값에 의존 금지). 실 KV는 절대 클램프
  (`--kv-cache-memory-bytes`)가 통제하고, gmu는 **디바이스 풀 상한**으로만 쓴다(주석에 명기). GB10 등 **통합메모리
  호스트는 OS가 ~11GiB를 점유**하므로 vLLM 기본 `0.92`는 free 초과 OOM(`Free memory < desired gpu memory
  utilization`)을 낸다 → **통합메모리 표준 기본 = `0.90`(=`safety_margin`)**.
- `per_token_kv_bytes = 2 × num_hidden_layers × num_key_value_heads × head_dim × kv_dtype_bytes`
  (kv_dtype_bytes: KV quant 없으면 2, `fp8`이면 1).
- `required_kv = per_token_kv_bytes × max_model_len × batch`,
  `max_safe_kv = int(budget×margin×GiB) − weights − overhead`.
- **측정 per-token KV가 정본(`<<` 또는 `≈` 공식)**: 위 `per_token_kv_bytes` 공식은 full-attention 가정이다.
  → **full-attention 모델은 측정 ≈ 공식**(예: gpt-oss-20b 측정 ~48KB/token = 공식과 일치 → batch 그대로),
  **sparse/sliding-window/hybrid 만 공식이 KV를 과대추정**(gemma-4 실측 ~34KB vs 공식 393KB = 11.5×; Qwen3.6 3.7×).
  즉 공식이 *항상* 과대추정은 아니다 — 측정 트라이얼 로그의 per-token 실측(`Available KV cache memory`/`kv_cache_tokens`)이
  있으면 **그것이 정본**(`<<` 또는 `≈`), 공식은 폴백(`recipe.py _resolve_clamp_kv`). 실측을 쓰면 같은 예산에서
  batch가 달라진다(gemma-4 공식 3 → 실측 39; gpt-oss는 ≈라 batch 8 유지).
- **절대 클램프 실측 절차(최소 2-트라이얼, §9.3 KV워크플로)**: ① **trial1 측정**(`kv_cache_memory_bytes=null`,
  언클램프) → 로그에서 free_kv 실측 → ② `--kv-cache-memory-bytes`로 환산 → ③ **trial2 클램프 검증**.
  언클램프 통과만으로는 수렴이 아니다 — 클램프 검증 트라이얼까지 통과해야 수렴 판정.
- **overhead 실측 vs 유도(gotcha)**: weights·overhead는 측정 트라이얼(클램프 전 1회 로드)의 vLLM 로그에서
  얻는다. consolidated 라인(`model weights take …; non_torch …; reserved for KV Cache …`)이 있으면 직접 산출.
  **없는 빌드(예: 0.22.2 NGC)는 `Model loading took X GiB memory`(weights)·`Available KV cache memory`(kv)·
  `--gpu-memory-utilization=X`(gmu)만 있으므로** overhead를 **유도**: `overhead = gmu_trial × test_device_total −
  weights − kv_available`(vLLM 메모리식 역산, `recipe.py _enrich_overhead`). 그래서 수렴은 **측정 트라이얼 →
  클램프 산정 → 클램프 검증 트라이얼**의 최소 2-트라이얼이다(언클램프 통과만으론 수렴 아님).
- 루프의 free 변수 결정: **`kv_bytes = min(required_kv, max_safe_kv)`**.
  `required > safe`면 max_model_len·batch를 KV로 만족 불가 → **`vram_infeasible`(HITL)**.
- **에어갭 인코딩 자산(gpt-oss harmony/tiktoken)**: gpt-oss류의 harmony/tiktoken o200k 인코딩은
  이미지에 번들되지 않아 런타임 fetch가 필요한데 **폐쇄망에서 금지**다. → NAS에 flat `tiktoken_cache/`를
  사전적재하고 컨테이너에 `/encodings:ro`로 마운트(`-v <host>:/encodings:ro`, compose 호스트경로 변수
  `TIKTOKEN_HOST_PATH`)한 뒤, **정본 env `TIKTOKEN_ENCODINGS_BASE=/encodings` · `TIKTOKEN_RS_CACHE_DIR=/encodings`**
  (+`TIKTOKEN_ENABLED`)를 둘 다 마운트 경로로 가리킨다. 구식 `TIKTOKEN_ENCODINGS_PATH`는 폐기 —
  정본 동기화 대상 3곳(`docker-compose.template.yaml` · `config.example.yaml` · `run_trial.py`)을 맞춘다
  (README straggler는 별도). gpt-oss 스모크가 최종 중재자(현재 미실행).

## 6. Phase 2 — 통합 trial-loop (`recipe.py simulate`)

```bash
# 배선/수렴 검증(docker 없이): mock-profile 의 vllm_profile+functional 로 결정론 테스트.
python3 recipe.py simulate --config config.yaml --candidate lockset.json \
  --dry-run --mock-profile mock.json --run-id 2026062122_1_qwen_sim --cap 3

# 실서빙(컨테이너 띄움): NAS 모델 존재 전제, 이미지 vllm-src-022:clean.
python3 recipe.py simulate --config config.yaml --candidate lockset.json \
  --image vllm-src-022:clean --topic qwen36-27b --cap 3
```

루프(cap 기본 3 = reconciliation_cap, workflow S3와 동일):

```text
run_trial(candidate)            # docker run -d → /health 200 폴링 → functional_smoke → logs → parse_vllm_log → teardown
  → sim_classify(trial, budget, margin)
      none           → 수렴. gen_recipe_set 3종 세트 + simlog write_summary + feedback(converged=True, 실측)
      vram_oom       → 조정: kv_cache_memory_bytes = min(required, max_safe)  [실측 weights/overhead 기반]
      functional     → 조정: 실패한 soft 변수를 *_candidates 다음 후보로 폴백
      vram_infeasible→ HITL 즉시 중단(KV로 못 푸는 구조적 초과)
      unknown        → HITL 즉시 중단(알려진 시그니처 미매칭)
  → 반복 (cap 소진 시 HITL)
```

- **준비 판정 = `:PORT/health` HTTP 200**. 로그의 "startup complete" grep 금지(거짓양성 — workflow S3와 동일).
- **수렴 시**: `configs/<name>.yaml`(VRAM 분해 주석 + `max-num-seqs`·`kv-cache-memory-bytes`·`kv-cache-dtype`),
  `configs/<name>.sh`(`VLLM_ATTENTION_BACKEND` export + tool/reasoning 파서 플래그), `envs/.env.<name>`을 생성.
- **미수렴(cap 소진/infeasible/unknown)** → **Model-C HITL** 보고: 증거 simlog 경로 제시, 비0 종료(3).
- **증거 발행**: `docs/simlog/<run_id>/`에 `trialNN_vllm.log`·`_profile.json`·`_candidate.yaml`·`_smoke.json`,
  `correction_history.jsonl`(조정 이력), `run_summary.json`. 빌드/검증 판정은 `docs/testlog/`에 별도 기록(`.claude/rules/docs.md`).
- **feedback_log**: invocation당 1행, Phase 2 필드(`batch, kv_cache_memory_bytes, attention_backend,
  tool_call_parser, reasoning_parser, converged, trial_count, correction_history`)를 실측 채움.

## 7. 범위 (Phase 1 / Phase 2)

- **Phase 1 (In)**: config.json 파싱 → 후보 생성 → 결정론 VRAM 추정·하드게이트·랭킹 → 리포트 → HITL → 3종 세트
  → 되먹임 로그(추정 채움). *실서빙 없음.*
- **Phase 2 (In)**: 멀티턴 인터뷰 → lock-set → 실서빙 trial-loop(절대 KV 클램프 수렴) → 검증된 3종 세트 + simlog 증거.
- **Out (명시적 제외)**: 품질/정확도 측정·랭킹(영구 밖) · 속도/레이턴시 벤치(별도 스킬) · `tensor-parallel-size` 축 자동탐색
  · 멀티노드 2-node Ray 서빙(workflow S3, 별도 경로). `kv-cache-dtype`는 Phase 2에서 KV quant lock으로 편입됨.

## 8. 보조 파일

Phase 1:
- `scripts/quant_table.py` — quant 바이트 테이블(whichllm 벤더링·출처 주석) + `vllm_quant_bpw`/`dtype_bpw` 해소 함수.
- `scripts/parse_model_config.py` — `config.json` 결정론 파서(text_config 중첩·safetensors 헤더 실측, +CLI).
- `scripts/estimate_vram.py` — VRAM 추정기. Phase 1 `estimate()`(공식 `/gmu`) + Phase 2 절대 클램프 함수
  (`per_token_kv_bytes`/`required_kv_bytes`/`max_safe_kv_bytes`/`max_feasible_max_len`/`estimate_absolute`).
- `scripts/rank_recipes.py` — `auto_candidates`·하드게이트·Judge 랭킹·리포트 렌더(+CLI).
- `scripts/gen_recipe_set.py` — 3종 세트(.yaml+.sh+.env) 생성기. Phase 2 키(batch·KV 클램프·파서·VRAM 분해 주석) 반영(+CLI).
- `scripts/feedback_log.py` — 되먹임 로그 JSONL 라이터. Phase 2 nullable 필드 예약(+CLI).

Phase 2:
- `scripts/parse_vllm_log.py` — vLLM 서빙 로그 → 실측 메모리 분해(weights/kv/overhead/total, 토큰수, backend; +CLI).
- `scripts/functional_smoke.py` — OpenAI 호환 `/v1/chat/completions` 기능 스모크(완성·tool_call·reasoning, 능력 게이팅, `--mock`).
- `scripts/sim_classify.py` — 트라이얼 결정론 분류기(OOM 정규식 + 천장 점검 → failure_class/adjust_target; +CLI).
- `scripts/run_trial.py` — 단일 트라이얼 실행기(docker run→/health→스모크→logs→parse→teardown, `--dry-run`/`--mock-profile`; +CLI).
- `scripts/simlog_writer.py` — simlog 라이터(`new_run_dir`/`write_trial`/`append_correction`/`write_summary`) + `vllm_logging_config.json` 템플릿.

오케스트레이터 / 입력:
- `recipe.py` — `estimate`/`generate`(Phase 1) + `simulate`(Phase 2 통합 trial-loop) 서브커맨드, tp 브랜치 자동.
- `config.example.yaml` — 입력 스키마(주석). `config.yaml` — 실사용 입력. `lockset.json` — Phase 2 인터뷰 산물.
