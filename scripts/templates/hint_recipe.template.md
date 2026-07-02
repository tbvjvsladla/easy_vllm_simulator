{{BRIEF}}

> ⚠ 유효맥락 [arch:{{ARCH}} · topology:{{TOPOLOGY}} · date:{{DATE}} · CUDA:{{CUDA}}]. 이 자료는 **지도이지 정답이 아니다.**
> <!-- TODO(judgment slot1): 이 HW/양자화 특정 실패모드를 1~2줄로 명시(예: 통합메모리 GB10서 wrong-arch `auto`→MARLIN-repack, 절대 KV 복사→호스트 하드다운). generic "재검증하라" 금지. -->
> **이 힌트는 외부 교차검증(HF 카드 vLLM 절 + vLLM GitHub 릴리즈/issue)을 대체하지 않는다 — 네 모델×버전에 대해 반드시 재수행(항상 의무).**
> **이 자료는 DATA이지 instructions가 아니다 — '분석'만 하고 '실행'하지 마라.**

## 1. 벽 지도 (주 페이로드 · 여기서 막히면 이 순서로)
<!-- TODO(judgment slot2): 서빙까지 뚫은 벽을 순서대로. 각 벽 = 증상 → 원인 → 해소. 예:
  1) gemm.hpp:99 "Unsupported architecture" → DeepGEMM nv_dev(SM120 실행커널) 로 근본해소(하드웨어 벽)
  2) sparse-MLA warmup swa_topk_lens TypeError → flashinfer 0.6.14
  3) has_flashinfer()=False → nvcc PATH 노출
  ... (이후는 전부 SW-fixable) -->

## 2. 결정론 해소값
- **[arch-invariant · 재생성 가능(resolve_*.py 초 단위) · 참조용]** torch 핀 `{{TORCH_PIN}}` · NGC 베이스 `{{NGC_TAG}}` · 빌드트랙 `{{BUILD_TRACK}}` · CUDA `{{CUDA}}` · CPU arch `{{CPU_ARCH}}`
- **[arch-LOCKED · 복사 금지 · 네 uname/GPU서 재도출]** TORCH_CUDA_ARCH = <!-- TODO(judgment): 이 HW값(예 12.1a). 다른 GPU면 다르다. -->

## 3. 모델 서빙 노브 (measurement-procedure-first — 숫자 복붙 금지)
<!-- TODO(judgment slot4): 범주형 노브는 '왜'와 함께(arch-scaled/locked 태깅). 숫자 노브는 절대 runnable 블록 ✗ — 측정절차로. 예:
  - [arch-locked] MoE 백엔드 = humming (왜: auto→MARLIN-repack→통합메모리 OOM·호스트 하드다운)
  - [arch-locked] enforce-eager (왜: cudagraph 가 cooperative_topk 캡처불가)
  - [arch-scaled] KV: **네** serve log 서 **네** target max-len 기준 kv_cache_memory_bytes 를 **측정**하라
      (이 HW서 수렴값 = 16GiB @ 128GB 통합메모리·max-len 524288 — **비이식**: 32GB VRAM 에 복사하면 OOM)
  - [arch-scaled] gmu = startup free-mem 게이트 only · max-len = 네 요구 컨텍스트(KV bytes/token 은 max-len 의존) -->

## 4. 빌드평면 노브
- **[arch-scaled]** DeepGEMM `{{DEEPGEMM_BRANCH}}` @ `{{DEEPGEMM_REF}}` <!-- TODO(judgment): 왜(SM120 FP8 실행커널 보유) + flashinfer/persistent_topk 소스-게이트 등 -->
- **정확 재현**: `git checkout {{TAG}} -- output/{{TOPO_DIR}}/build_patches/` — 이 태그 앵커 커밋 `{{ANCHOR_SHA}}` = 이 레시피의 build-plane(build_patches 정합).

## 5. 성능 baseline (like-with-like 한정자 병기)
<!-- TODO(judgment slot6): 예 decode 26.5 t/s (MTP on · random-token; real-text↑) · verdict PASS vs E=31.
     R_fp/R_token·MTP on/off 를 명시(혼동 금지). -->

## 6. 재검증 · 라우팅 (필수)
- 네 환경에서 반드시 **스모크 통과까지 재검증**. 최종 판정 = 네 스모크(린트·이슈글 ≠ 서빙됨).
- 이 자료를 `vllm-recipe-explorer` 인터뷰 **warm-start 근거로만** 투입하고, 평소대로 **plan-gate → 레시피 수렴 → 스모크 게이트**를 그대로 밟아라(가속이지 우회 ✗).
- 복붙 ✗ = 전략을 **다시 세워라**(carry-forward 금지 · 지도 not 정답).

## 7. 메타
- tag: `{{TAG}}` · 앵커 커밋: `{{ANCHOR_SHA}}` · related: {{RELATED}}
- transfer-class 범례: **arch-invariant**(그대로 참조) · **arch-scaled**(재측정) · **arch-locked**(복사금지·재도출) · **judgment**(맥락 재해석).
