# Phase 1 — 입력·파싱·교차검증·랭킹 상세 (조건부 reference)

> spine step 2(parse)·step 3(crosscheck)·step 4(후보 생성)·step 5(rank/HITL)·step 6(generate)의 **세부**.
> SKILL.md 는 순서와 안전경계만 들고 있고, 필드 의미·함정·근거는 여기서 읽는다.

## 0. 입력 (`config.yaml`)

사람이 미리 작성한 `config.yaml`(영속)을 읽는다. 없으면 `config.example.yaml` 을 참고해 요청한다.

- `target_model.path`: 대상 모델의 **컨테이너 경로**(`/app/models/<Org>/<Name>`, `configs/*.yaml` 의 `model:` 과 동일).
- `nas_host_root`: `/app/models` 가 매핑되는 호스트 NAS 루트(기본 `/mnt/models` — 실제 값은 `config.yaml`).
- `vram_budget_gb`: carve-out 가상 GPU 예산(예: RTX4090=24).
- `safety_margin`: 하드게이트 임계(기본 0.90).
- `kv_cache_dtype_bytes`: KV dtype 바이트(기본 2=fp16).
- `test_device_total_gib`(Phase 2): 측정 하드웨어 total VRAM(GB10=121.69, torch.cuda 기준). consolidated
  메모리 라인이 없는 vLLM 빌드에서 overhead 유도(`gmu×total−weights−kv`)에 쓴다. DGX Spark 는 nvidia-smi 가 N/A.
- `tensor_parallel_size`(선택): 미지정 시 **manifest 배선**(`len(nodes)×gpus_per_node` · nodes 비면 topology=single→1 ·
  최종폴백 1 — **git 브랜치 폴백 ✗**: 브랜치⇒TP 가 보고된 버그였음). 명시 override > manifest > 1.
- `serving.{config_name, port, served_model_name}`: 생성될 3종 세트의 base 이름·포트·서빙명.

**NAS 마운트 불변식(포인터 원칙)**: 컨테이너 NAS 마운트 경로를 스크립트에 하드코딩하지 않는다 — 호스트 NAS 루트 해소
(recipe 호스트파싱 평면) = `config.yaml.nas_host_root > env(NAS_MODEL_PATH) > manifest.nas_model_path > DEFAULT(/mnt/models)`
(`_cfg_common`·`run_trial` 파라미터화 — 유지해야 할 불변식) → `/app/models` 에 매핑. 컨테이너 경로 ↔ 호스트 경로 변환은
이 4-tier 포인터로만 이뤄진다(manifest = 단일 권위).

## 1. 결정론 / 확률론 경계

| 구성요소 | 방식 | 비고 |
|----------|------|------|
| config.json 파싱 | **결정론 스크립트** | `parse_model_config.py` — text_config 중첩·safetensors 헤더 실측 |
| **모델카드 교차검증**(§3) | **결정론** | `crosscheck_model_card.py` — 번들 README(HF 원본카드)+inference/reqs+dtype 실측 ↔ config 합치 + 외부(HF API) 파라미터-총계 이중검증. coarse-quant 함정·special-dep 사전경보·`du -sh` 오염 방지. MISMATCH=비0(게이트) |
| 후보 **생성**(brainstorm) | **LLM (이 단계만 확률론)** | 3축 조합 다양성이 가치(Generate&Filter 의 Generator) |
| VRAM 추정 **공식**(per-token KV) | **결정론 — 단 상한(upper bound)** | `estimate_vram.py` full-attention 가정 공식. sliding-window/GQA서 **과대추정**(gemma 8×·gpt-oss 1.9×) → OOM 보수 게이트엔 유효, near-max batch엔 **부정확** |
| VRAM **실측 분해**(near-max 정본) | **결정론 — 측정 정본** | serve KV log(`kv_cache_tokens`/`max_concurrency`) 또는 Phase-2. **near-max batch·절대 KV 클램프는 측정으로만** |
| 하드 안전 게이트(margin) | **결정론** | 예산×margin 초과 후보 탈락(zero tolerance) |
| Judge 랭킹(headroom→context) | **결정론** 정렬 | 품질·속도 인자 없음 |
| 3종 세트 생성 | **결정론** 템플릿 | 기존 워크스페이스 스키마 준수 |
| **인터뷰** | **LLM (확률론)** | Phase 2. soft 변수에 `*_candidates` 폴백 리스트를 채움 |
| **VRAM 로그 파싱** | **결정론** | `parse_vllm_log.py` — 정규식, 라인 부재 시 null |
| **트라이얼 분류** | **결정론** | `sim_classify.py` — 알려진 OOM 정규식 + 천장 점검 |
| **KV 클램프 재산정** | **결정론** | `recipe.py simulate` 루프 — 실측 weights/overhead 기반 |

→ "**결정의 책임은 결정론 계층이 진다. 탐색·인터뷰만 LLM.**"

## 2. ① parse (결정론)

```bash
python3 scripts/parse_model_config.py <path> [--nas-root R] [--json] > parsed.json
```

- `/app/models/...` 입력은 `nas_host_root` 로 치환해 호스트 NAS 에서 읽는다.
- **text_config 중첩 처리**: `text_config` 가 있으면 아키텍처 필드를 그쪽 우선·top-level fallback 으로 읽는다
  (`vision_config` 는 무시). 예: Qwen3.5-35B-A3B 는 전 필드가 `text_config` 아래.
- **num_params 산출(로컬 파싱, 결정론)**: 비prequantized 모델은 **index 가 가리키는 정본 샤드들의 safetensors 헤더에서
  텐서별 numel(shape 곱)을 합산** — 저장 dtype 과 무관한 정확한 파라미터 수다(config `torch_dtype` 이 거짓이거나
  [Motif-2.6B: 디스크 fp32지만 bf16 모델], 한 디렉토리에 복수 정밀도 체크포인트가 공존해도[LFM2-8B-A1B] 정확).
  prequantized(mxfp4 등)는 `quant_method_native` bpw 로 native weight 를 나눈 best-effort. `native_weight_bytes` 는 index
  `total_size`(없으면 파일 크기 합), `disk_bpw` 는 최대 safetensors 헤더의 온디스크 dtype 실측(폴백·보고용).
- 디렉토리/`config.json` 부재 → 명시적 에러(비0 종료, 다운로드 금지).

## 3. ①.5 모델카드 교차검증 (결정론 · serving 착수 전 필수 루틴)

```bash
python3 scripts/crosscheck_model_card.py <path> [--hf-repo-id <org/name>] [--json]   # managed(로컬) — MISMATCH 시 비0 종료(게이트)
python3 scripts/crosscheck_model_card.py --ephemeral-estimate --hf-repo-id <org/name> [--json]   # ephemeral(다운로드 전 사전추정)
```

- **HF 원본 모델카드(번들 `README.md`) + `inference/requirements.txt` + config.json + safetensors dtype 실측**을 교차대조 →
  config.json **단독** 파싱이 놓치는 사실을 serving *전* 노출.
- 잡는 것: **① coarse quant 라벨 함정**(config `quant_method:fp8` 인데 실측 experts=FP4 혼합) ·
  **② novel-arch special-dep**(DeepGEMM·tilelang·flash_attn… → "vLLM dry-init/op 가용성 확인" 권고) · 정밀도·파라미터·컨텍스트·아키·reasoning 카드 합치.
- **③ 외부(HF API) VRAM 이중검증**: `du -sh` 류 디렉토리 전체크기가 `.git`(HF LFS 캐시) 오염으로 실제 가중치의 최대 2배까지
  부풀 수 있음이 실증됨(2026-07-08, gemma-4-E2B-it 20GB→실 9.54GiB) — **모델 용량 판단에 `du -sh` 를 근거로 쓰지 않는다**.
  `--hf-repo-id` 지정 시 HF 공개 API(`GET /api/models/<repo_id>` — 모델 다운로드 없이 `safetensors.total` 조회)로 로컬 실측과
  대조하거나, `--ephemeral-estimate`(다운로드 전) 로 파라미터 총계만으로 사전추정한다. 조회 실패는 음성정직(verdict=UNAVAILABLE).
- **special-dep 분류·핸드오프 (발견≠소유 — per-model 3+1+1, plan_26062812)**:
  · **빌드-바깥 의존**(native lib/커널: DeepGEMM·tilelang·flash_attn…) = **patch.py ✗ · recipe-explorer 자체수정 ✗** →
  **`upstream-version-watch` 핸드오프**(빌드 평면 — `build_patches/<NN>-*.sh`; 메인) / **서브면 docs insight 상향**(D12).
  · **런타임-코드 불일치**(Python processor/config shim) = `<model>_patch.py`(`kv-clamp.md` §런타임 패치).
- **근거(실증)**: config coarse `fp8` + `du` 아티팩트만 봐 DeepSeek-V4-Flash 를 순수FP8/298GB/인피저블로 오판 →
  카드·실측은 `FP4+FP8 mixed`/149GiB(적합). MISMATCH/WARN 은 **HITL surface**(자동 무시 금지).

## 4. ② LLM 후보 생성 (이 단계만 확률론)

모델 특성(`is_moe`, `num_params`, `max_position_embeddings`, prequantized 여부)을 고려해 3축 조합을 brainstorm 한다.

- `quantization`: prequantized 면 **native 고정**(`quant_method_native`). 비prequantized 면 `{none, fp8}`
  (비prequantized 체크포인트에 `awq`/`gptq` 를 제안할 땐 **경고 표기** — 런타임 양자화 불가).
- `max-model-len`: **4096부터 `max_position_embeddings` 까지 2의 거듭제곱**. 미상이면 `[4096, 8192, 16384, 32768]`.
- `gpu-memory-utilization`: 0.80~0.95(기본 그리드 `[0.85, 0.90, 0.95]`).

생성 후보를 `candidates.json` 으로 저장 → `recipe.py estimate --candidates candidates.json`. 결정론 기본 그리드만 쓰려면 `--auto`.

## 5. ③ rank (결정론 하드게이트 + Judge) → 리포트 → ③.5 HITL

```bash
python3 recipe.py estimate --config config.yaml --auto
```

- 각 후보 → `estimate_vram.py` 로 **`(weight_bytes/tp + KV_cache + overhead) / gpu_memory_utilization`** 추정.
  KV·overhead 는 `/tp` 하지 않음 = 멀티노드 보수적 과대추정 → OOM 게이트에 안전.
- **하드게이트**: `estimated_total_gib ≤ vram_budget_gb × safety_margin` 초과 후보 전부 탈락(zero tolerance).
- **Judge**: 통과 후보를 `headroom_gib` 내림차 → `max_model_len` 내림차로 정렬(id 보존). ranked 결과는 `feedback/.last_ranking.json`.
- **③.5 HITL**: 사람이 `recipe_id`(예 `r3`)를 고른다. 무feasible(전부 FAIL)이면 예산 상향/마진 완화/모델 변경을 보고(자동 강행 금지).
  **단 "전부 FAIL" 보고 전 의무 2단계**(공식-only 인피저블 선언 금지 — 공식은 최대 8× 과대추정하는 상한일 뿐):
  (a) **측정 보강 제안을 기본 경로로** — Phase-1.5 1회 serve 또는 Phase-2 trial 로 실측 per-token KV 확인을 먼저 제안,
  (b) **외부 교차검증** — `.claude/rules/references.md` §5 레시피로 "정말 못 띄우는 모델인지" 확인·기록(testlog "탐색 증거").
  둘 다 없이 인피저블 단정 ✗.

## 6. ④ generate (결정론 3종 세트) + 되먹임 로그

```bash
python3 recipe.py generate --config config.yaml --recipe-id r3
```

- **출력 통로 = `output/<topology>/{configs,envs}/`**(compose 가 마운트하는 통로와 정합 — 결함#3, `testlog_26062422`).
  - `configs/<name>.yaml` — `model: <container_path>`, `host 0.0.0.0`, `port 8000`, `gpu-memory-utilization`/`max-model-len`,
    `quantization` 은 **native/none 이 아닐 때만**.
    ⚠ **Phase-1(estimate→generate)은 `max-num-seqs`(near-max batch)·`kv-cache-memory-bytes`(절대클램프)를 emit 하지 않는다**
    (rank_recipes 는 batch 미산정 — 결함#4). 이 둘은 **Phase-2(simulate) 측정 산물**이다.
  - `configs/<name>.sh` — TIKTOKEN 가드 + `vllm serve --config ... --served-model-name`.
  - `envs/.env.<name>` — `COMPOSE_PROJECT_NAME·CONTAINER_NAME·VERSION·NVIDIA_VISIBLE_DEVICES·SERVING_IP/PORT·TIKTOKEN_ENABLED·SERVING_MODEL_NAME·CONFIG_FILE`.
  - 기존 파일이 있으면 `--force` 없이는 덮어쓰지 말고 에러.
- `feedback_log.py` 가 invocation 당 1행(JSONL)으로 append 한다(추정 필드 채움 + 실측 nullable 예약).
