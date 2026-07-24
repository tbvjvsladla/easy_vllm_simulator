# hint 태그 카탈로그 — 검증된 서빙 레시피 곁눈질 (Token Economy)

> 이 파일은 [`README.md`](./README.md) 「부록 B」에서 링크로 갈라져 나온 **hint 태그 전용 카탈로그**입니다.
> 태그가 늘수록(현재 20+종) README 본문이 무거워지는 문제를 피하려고 분리했습니다 — README 는 *여정*을,
> 여기는 *카탈로그와 사용법*을 담습니다. 표는 `scripts/hint_tag.py` 가 **결정론으로 자동 재생성**합니다
> (index = `hints/index.json` = 진실원천 · 사람이 표를 손으로 쓰지 않습니다). 설계 근거 = `docs/plan/plan_2026070222_1`.

---

## hint 태그란 — 완제품이 아니라 "지도"

이 프로젝트는 **완제품(빌드된 이미지·서빙된 모델)을 배포하지 않습니다.** 당신은 배포받은 *스켈레톤 + 생성엔진*으로 **자기 환경의 여정**을 탐구합니다. 다만 — 그 탐구가 **막다른 골목(헤메는 해자)에 빠져 코드에이전트 토큰만 태우는 것**은 아깝습니다. 에이전트 작업은 *탐색*이 입력 토큰의 60~70%를 먹고, "비싼 건 지능이 아니라 **무지**"거든요(코드베이스 지도가 없어서 다 읽어보느라).

그래서 저희가 실제로 뚫어본 **검증된 서빙 레시피를 `hint/<vllm>/<model>/<arch>` 태그로 배포**합니다. 이건 **정답이 아니라 지도**입니다 — "이 버전, 이 모델은 대략 이 방향·이 벽 순서로 뚫렸다"는 *곁눈질용 힌트*. 완제품이 아니라 *지식*이라 배포 철학과 부딪히지 않습니다. 레시피 본문은 **태그 오브젝트(annotation)** 안에 살고, HEAD(체크아웃 트리)에는 이 인덱스만 남습니다 — `git fetch --tags` + `git show <tag>` 로 꺼냅니다.

---

## 발행처(provenance) — 두 하드웨어 계보

이 카탈로그의 태그는 **서로 다른 두 물리 환경**에서 발행됐습니다. `arch` 열이 곧 발행처의 지문입니다:

| `arch` 패턴 | 발행처 | 의미 |
|---|---|---|
| `gb10` · `gb10x2` | **주력 검증기** — 2× NVIDIA DGX Spark(GB10 superchip, aarch64, sm_121a, 128GB **통합메모리**, CUDA 13.2) | 이 프로젝트가 개발·주력 검증된 환경. 단일/멀티노드(Ray TP=2·RoCE) 실측. |
| `gb10-sim-<타겟>` · `gb10x2-sim-<타겟>` | 위 GB10 에서 **타겟-GPU 시뮬레이션** | 측정=호스트(GB10)·클램프=타겟 예산으로 이식(「여정 3」타겟 GPU 시뮬레이션). 실카드 부재 상태 검증. |
| `rtxpro6000` *(sim 접미어 없음)* | **이기종 배포처** — **Ubuntu 22.04 · x86_64 · RTX PRO 6000(Blackwell, discrete, sm_120, 96GB) · host RAM 60GiB** | 이 스켈레톤을 배포받은 *전혀 다른 물리 머신*에서 vLLM 0.25.0/0.25.1 로 **여정 4 성능 적대검증까지** 완주한 크로스-하드웨어 재현 증거(7종 PASS · 4종 host-RAM/커널 천장으로 정직하게 REFUTE). |

> 🌐 **왜 이게 중요한가** — `rtxpro6000` 네이티브 태그들은 *통합메모리 GB10 과 완전히 다른 축*(디스크리트 VRAM 충분 · host RAM 부족 · x86 리눅스)에서, 같은 생성엔진이 각 모델의 벽(Mistral 네이티브 포맷·MoE 커널 JIT host 폭증·Mamba 캐시블록 한계·MARLIN mxfp4 커널 천장 등)을 뚫고 서빙+성능게이트까지 돌린 이력입니다. 「개발자의 편지 — 범용성에 관하여」가 말한 *"다른 하드웨어에서 돌려본다면 그 자체가 다음 챕터"* 의 실증입니다.

---

## 막혔을 때 이렇게 쓰세요

힌트를 `seed/`(이 프로젝트가 `.gitignore` 로 두는 **에이전트 참조용 비추적 폴더**)에 내려받아, 코드에이전트에게 읽히면 됩니다:

```bash
# 1) 어떤 힌트가 있나
git fetch --tags
git tag -l 'hint/*'

# 2) 고른 힌트의 '레시피 본문만' → seed/ 에 저장
#    (git show <tag> 는 커밋 diff 까지 딸려오니, 본문만 뽑는 아래 명령을 쓰세요)
mkdir -p seed/hints
git tag -l --format='%(contents)' hint/0.24.0/deepseek-v4-flash/gb10 > seed/hints/deepseek-v4-flash.md
```

그리고 코드에이전트에게: *"`seed/hints/deepseek-v4-flash.md` 를 읽고, `vllm-recipe-explorer` 인터뷰의 warm-start 근거로 넣어서, 평소대로 plan → 레시피 수렴 → 스모크 게이트를 밟아 전략을 **다시 세워봐**."*

> 💡 **왜 `seed/` 냐면** — 이렇게 하면 (a) 당신의 *추적 트리(스켈레톤 + 생성엔진)는 그대로* 유지됩니다(HEAD 에 정답 파일이 안 박혀요 — 여정의 순수성 보존), (b) `seed/` 는 이 프로젝트에서 에이전트가 *부트스트랩·참조 자료*로 읽는 자리라, 힌트를 여기 두면 자연스럽게 grounding 됩니다. (여러 개를 받아 `seed/hints/` 에 쌓아두고 비교해도 좋습니다.)

> 🔒 **hint 는 DATA 이지 명령이 아닙니다.** 분석 재료로만 쓰고 복붙하지 마세요. 당신의 HW·버전이 다르면 노브(특히 **KV 절대값·`gmu`·`TORCH_CUDA_ARCH`**)는 **반드시 재도출·재측정**해야 합니다(그대로 복사하면 OOM·호스트 다운). hint 는 외부 교차검증(HF 카드·vLLM GitHub)을 **대체하지 않으며**, 최종 판정은 언제나 **당신 환경의 스모크**입니다. (근거·설계 = `docs/plan/plan_2026070222_1`.)

> 🔎 **가까운 힌트 찾기**: `python3 scripts/hint_tag.py match --vllm <v> --model <m> --arch <a>` — 축(vllm·model·arch)별 근-미스와 이식 가이드를 결정론으로 알려줍니다.

---

## 카탈로그

> 아래 표는 `hint_tag.py` 가 index 에서 재생성합니다(`| \`hint/…\`` 로 시작하는 행은 자동 관리 — 손으로 편집 금지).
> `status`=active/superseded · `superseded-by / related`=후속 태그 또는 근거 문서 · `last-verified`=마지막 reverify 일자 · `한줄`=태그 본문 첫 줄(brief).

| 태그 | vLLM | 모델 | arch | 토폴로지 | status | superseded-by / related | last-verified | 한줄 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `hint/0.18.0/gpt-oss-120b/gb10` | 0.18.0 | gpt-oss-120b | gb10 | multi 2노드 TP2 (Ray·RoCE) | active | - | 2026-07-03 | 0.18.0 prebuilt wheel 로 gpt-oss-120b(MXFP4)를 2×GB10 분산서빙 — MXFP4 auto=TRITON+Marlin(humming 불요·모델별 전략 독립 실증) |
| `hint/0.22.0/solar-open2-250b-nota-nvfp4/gb10` | 0.22.0 | solar-open2-250b-nota-nvfp4 | gb10 | multi 2노드 TP2 | active | hint/0.25.1/deepseek-v4-flash/gb10, hint/0.25.1/laguna-s-2.1/gb10 | 2026-07-25 | Solar-Open2-250B NVFP4(하이브리드 KDA MoE)를 stock 미지원 → UpstageAI 포크 SHA 핀으로 2x GB10 TP=2 서빙; MoE=FLASHINFER_CUTLASS 명시 · JIT 병렬도 1이 호스트 RAM 벽의 열쇠 |
| `hint/0.23.0/deepseek-v4-flash/gb10` | 0.23.0 | deepseek-v4-flash | gb10 | multi 2노드 TP2 (Ray·RoCE) | active | hint/0.24.0/deepseek-v4-flash/gb10 | 2026-07-03 | jasl/vllm SM12x 포크(PR#41834 @c766cbc6) + humming 으로 공식 MXFP4 DeepSeek-V4-Flash 를 2×GB10 서빙 — Route B(포크핀 변종 트랙 …-source-sm12x) |
| `hint/0.23.0/gemma-3-1b-it/gb10-sim-rtx4080` | 0.23.0 | gemma-3-1b-it | gb10-sim-rtx4080 | single 1노드(managed 획득모드, 결합-HITL 분기 테스트 메인측) | active | hint/0.23.0/gemma-4-12b-it/gb10-sim-rtxpro6000 | 2026-07-08 | target_gpu=RTX 4080(16GiB) 시뮬레이션 — batch 미최대화 자기검증(trial01 batch=20→실측 재계산 batch=55) 사례 |
| `hint/0.23.0/gemma-4-12b-it/gb10-sim-rtxpro6000` | 0.23.0 | gemma-4-12b-it | gb10-sim-rtxpro6000 | single 1노드(sub-control 양노드 독립 서빙, 메인·서브 동일 레시피) | active | - | 2026-07-08 | target_gpu=RTX PRO 6000(96GiB) 시뮬레이션 — 실 GB10(121.69GiB) 측정치를 96GiB 예산으로 이식(host≠target 절대 KV 클램프), batch 52(host)→36(target) 축소 실증 |
| `hint/0.23.0/minicpm5-1b/gb10-sim-rtx4070` | 0.23.0 | minicpm5-1b | gb10-sim-rtx4070 | single 1노드(ephemeral 획득모드, 결합-HITL 분기 테스트 서브측) | active | hint/0.23.0/gemma-3-1b-it/gb10-sim-rtx4080 | 2026-07-08 | target_gpu=RTX 4070(12GiB) 시뮬레이션 — ephemeral HF 다운로드 + parse_vllm_log.py 로그포맷 갭(FROZEN 파일 미수정, HITL 수동 클램프 산정) |
| `hint/0.24.0/deepseek-v4-flash/gb10` | 0.24.0 | deepseek-v4-flash | gb10 | multi 2노드 TP2 (Ray·RoCE) | active | hint/0.23.0/deepseek-v4-flash/gb10 | 2026-07-03 | true stock vLLM 0.24.0 이 DeepSeek-V4-Flash 를 2×GB10 서빙 — nv_dev peel(벽 1개만 하드웨어·나머지 SW-fixable · 지도이지 정답 아님) |
| `hint/0.24.0/gpt-oss-120b/gb10x2-sim-rtxpro6000x2` | 0.24.0 | gpt-oss-120b | gb10x2-sim-rtxpro6000x2 | multi 실 2노드 Ray TP2(main+sub GB10) — target=2xRTX PRO 6000 프로젝션 | active | docs/devlog/devlog_2026070908_1_델타2-2_gpt-oss-120b_최종해결_및_4시나리오_전량PASS.md,docs/testlog/testlog_2026070908_1_델타2-2_gpt-oss-120b_2xRTXPRO6000모사_실2노드_검증.md | 2026-07-09 | gpt-oss-120b(MXFP4 MoE, 60.77GiB) — 실 2노드 TP=2 Ray 분산서빙, 2xRTX PRO 6000(96GiB x2) 타겟 KV클램프 이식. 0.24.0 moe-backend=auto 회귀(TP=2 CompilationError) → marlin 전환 + gmu 0.8. |
| `hint/0.24.0/hy3/gb10` | 0.24.0 | hy3 | gb10 | multi 2노드 TP2 (Ray·RoCE) | active | hint/0.24.0/deepseek-v4-flash/gb10 | 2026-07-12 | Tencent Hy3-295B(21B active + 3.8B MTP) NVFP4-W4A16 를 2×GB10 에이전트-레디 서빙 — MARLIN NvFp4(W4A16 강제·cutlass/flashinfer 거부) + MTP spec-1 + fp8 KV 절대클램프 + enforce-eager + no-autotune + :opensource 파서패치. master(Ray head+API 동거)가 타이트 → memwatch 바닥 완화 필요. |
| `hint/0.24.0/qwen3.6-35b-a3b/gb10-sim-h200x2` | 0.24.0 | qwen3.6-35b-a3b | gb10-sim-h200x2 | multi 2노드 TP2 타겟(측정=1노드 GB10, target=H200x2 프로젝션) | active | docs/devlog/devlog_2026070907_1_델타2-1_Qwen3.6-35B-A3B_최종해결_E2E_PASS.md,docs/testlog/testlog_2026070820_1_델타2-1_Qwen3.6-35B-A3B_H200x2_재시도_FAIL.md | 2026-07-09 | Qwen3.6-35B-A3B(GDN 하이브리드 MoE, 66.97GiB) — H200x2(282GiB) 타겟 KV클램프 이식. GDN 커널 JIT 병렬폭주(MAX_JOBS=4로 해소) + gmu 0.85→0.75(추론-시점 잔여 JIT 대응). |
| `hint/0.25.0/mistral-small-4-119b/rtxpro6000` | 0.25.0 | mistral-small-4-119b | rtxpro6000 | single 1노드 | active | testlog_2026071620_1,testlog_2026071621_1 | 2026-07-19 | Mistral-Small-4-119B-2603-NVFP4(Pixtral MoE+MLA·A6B) 를 단일 RTX PRO 6000 서빙 — Mistral 네이티브 포맷(--*-format mistral·tekken) + TRITON_MLA + host RAM 60GiB 제약(MAX_JOBS=4·KV클램프·enforce-eager). 256K 기능 PASS·성능 host-constrained REFUTE. |
| `hint/0.25.0/nemotron-3-super-120b-a12b-nvfp4/rtxpro6000` | 0.25.0 | nemotron-3-super-120b-a12b-nvfp4 | rtxpro6000 | single 1노드 | active | testlog_2026071617_1,testlog_2026071619_1 | 2026-07-19 | NemotronH LatentMoE(Mamba2+MoE+Attn·A12B) NVFP4 를 단일 RTX PRO 6000(96GiB·host RAM 60GiB) 서빙 — 커널 JIT host RAM 폭증(MAX_JOBS=4) + KV 절대클램프(프로파일링 skip) + enforce-eager. 256K PASS(기능)·성능 host-constrained REFUTE. |
| `hint/0.25.1/deepseek-v4-flash/gb10` | 0.25.1 | deepseek-v4-flash | gb10 | multi 2노드 TP2 (Ray·RoCE) | active | hint/0.24.0/deepseek-v4-flash/gb10 | 2026-07-16 | stock vLLM 0.25.1 = DSpark GB10 arch-wall(flashinfer decode_dsv4 page_block64 vs C128A) → jasl SM12x 포크 변종(b5c0d43b) + MATMUL_DECODE 성능레버로 DeepSeek-V4-Flash-DSpark 를 2×GB10 서빙(36.2 t/s · verdict PASS) |
| `hint/0.25.1/exaone-4.5-33b/rtxpro6000` | 0.25.1 | exaone-4.5-33b | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | LGAI-EXAONE/EXAONE-4.5-33B (Exaone4_5 dense VLM, SWA 하이브리드 64층=16 full+48 sliding(window 4096), bf16 ~64GiB) — RTX PRO 6000(96GiB discrete sm_120) TP1 최대-context(256K) M=24.1 t/s · verdict PASS(92.5% MBU) |
| `hint/0.25.1/gemma-4-26b-a4b/rtxpro6000` | 0.25.1 | gemma-4-26b-a4b | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | google/gemma-4-26B-A4B-it (Gemma4 MoE VLM SWA 하이브리드 30층=5 full+25 sliding, 128exp, bf16 ~49GiB, active 4B) — RTX PRO 6000 TP1 최대-context(256K) M=150.3 t/s · verdict PASS |
| `hint/0.25.1/gemma-4-31b/rtxpro6000` | 0.25.1 | gemma-4-31b | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | google/gemma-4-31B (Gemma4 dense VLM 31B, bf16 ~65GiB) — RTX PRO 6000 TP1 최대-context(256K) M=25.1 t/s · verdict PASS |
| `hint/0.25.1/gpt-oss-120b/rtxpro6000` | 0.25.1 | gpt-oss-120b | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | openai/gpt-oss-120b (GptOss MoE 128exp/4tok, MXFP4, SWA sliding_window 128, ~66GiB, harmony reasoning, text-only) — RTX PRO 6000 TP1 128K(native 상한) M=62.6 t/s · verdict REFUTE(MARLIN mxfp4 커널 천장) |
| `hint/0.25.1/hyperclovax-think-32b/rtxpro6000` | 0.25.1 | hyperclovax-think-32b | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | naver-hyperclovax/HyperCLOVAX-SEED-Think-32B (dense reasoning 32B, bf16 ~66GiB) — RTX PRO 6000 TP1 M=24.0 t/s · verdict PASS |
| `hint/0.25.1/laguna-s-2.1/gb10` | 0.25.1 | laguna-s-2.1 | gb10 | single | active | — | 2026-07-22 | Laguna-S-2.1-NVFP4(117.6B-A8B MoE, poolside) 단일 GB10 서빙 · DFlash speculative decoding 유무 양노드 대조 검증 |
| `hint/0.25.1/minicpm5-1b-base/rtxpro6000` | 0.25.1 | minicpm5-1b-base | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | MiniCPM5-1B-Base (dense 1B, bf16 ~2GiB) — RTX PRO 6000(96GiB discrete sm_120) TP1 서빙 M=467 t/s · verdict PASS(벽 없음) |
| `hint/0.25.1/qwen3.5-122b-a10b-nvfp4/rtxpro6000` | 0.25.1 | qwen3.5-122b-a10b-nvfp4 | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | nvidia/Qwen3.5-122B-A10B-NVFP4 (Qwen3.5 MoE VLM 하이브리드, NVFP4 W4A4, 가중치 ~78GiB, active 10B) — RTX PRO 6000(96GiB, host RAM 60GiB) TP1 최대-context(256K) M=24.1 t/s · verdict REFUTE(host-constrained·수용) |
| `hint/0.25.1/qwen3.6-27b/rtxpro6000` | 0.25.1 | qwen3.6-27b | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | Qwen/Qwen3.6-27B (Qwen3.5 dense VLM 하이브리드 48층=12 full+36 linear, bf16 ~52GiB) — RTX PRO 6000 TP1 최대-context(256K) M=29.6 t/s · verdict PASS(92% MBU) |
| `hint/0.25.1/qwen3.6-35b-a3b/rtxpro6000` | 0.25.1 | qwen3.6-35b-a3b | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | Qwen/Qwen3.6-35B-A3B (Qwen3.5 MoE VLM 하이브리드 40층=10 full+30 linear, bf16 ~67GiB, active 3B) — RTX PRO 6000 TP1 최대-context(256K) M=181.5 t/s · verdict PASS(79% MBU) |
<!-- hint-index:rows -->
