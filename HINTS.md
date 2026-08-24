# hint 태그 카탈로그 — 검증된 서빙 레시피 곁눈질 (Token Economy)

> 이 파일은 [`README.md`](./README.md) 「부록 B」에서 링크로 갈라져 나온 **hint 태그 전용 카탈로그**입니다.
> 태그가 늘수록(현재 20+종) README 본문이 무거워지는 문제를 피하려고 분리했습니다 — README 는 *여정*을,
> 여기는 *카탈로그와 사용법*을 담습니다. 표는 `.claude/skills/hint-publisher/scripts/hint_tag.py`가 **결정론으로 자동 재생성**합니다
> (index = `hints/index.json` = 진실원천 · 사람이 표를 손으로 쓰지 않습니다). 설계 근거 = `docs/plan/plan_26070222`.

---

## hint 태그란 — 완제품이 아니라 "지도"

이 프로젝트는 **완제품(빌드된 이미지·서빙된 모델)을 배포하지 않습니다.** 당신은 배포받은 *스켈레톤 + 생성엔진*으로 **자기 환경의 여정**을 탐구합니다. 다만 — 그 탐구가 **막다른 골목(헤메는 해자)에 빠져 코드에이전트 토큰만 태우는 것**은 아깝습니다. 에이전트 작업은 *탐색*이 입력 토큰의 60~70%를 먹고, "비싼 건 지능이 아니라 **무지**"거든요(코드베이스 지도가 없어서 다 읽어보느라).

그래서 저희가 실제로 뚫어본 **검증된 서빙 레시피를 `hint/<vllm>/<model>/<arch>` 태그로 배포**합니다. 이건 **정답이 아니라 지도**입니다 — "이 버전, 이 모델은 대략 이 방향·이 벽 순서로 뚫렸다"는 *곁눈질용 힌트*. 완제품이 아니라 *지식*이라 배포 철학과 부딪히지 않습니다. 레시피 본문은 **태그 오브젝트(annotation)** 안에 살고, HEAD(체크아웃 트리)에는 이 인덱스만 남습니다 — `git fetch --tags` + `git show <tag>` 로 꺼냅니다.

---

## 발행처(provenance) — 두 하드웨어 계보

이 카탈로그의 태그는 **서로 다른 세 물리 환경**에서 발행됐습니다. `arch` 열이 곧 발행처의 지문입니다:

| `arch` 패턴 | 발행처 | 의미 |
|---|---|---|
| `gb10` · `gb10x2` | **주력 검증기** — 2× NVIDIA DGX Spark(GB10 superchip, aarch64, sm_121a, 128GB **통합메모리**, CUDA 13.2) | 이 프로젝트가 개발·주력 검증된 환경. 단일/멀티노드(Ray TP=2·RoCE) 실측. |
| `gb10-sim-<타겟>` · `gb10x2-sim-<타겟>` | 위 GB10 에서 **타겟-GPU 시뮬레이션** | 측정=호스트(GB10)·클램프=타겟 예산으로 이식(「여정 3」타겟 GPU 시뮬레이션). 실카드 부재 상태 검증. |
| `rtxpro6000` *(sim 접미어 없음)* | **이기종 배포처** — **Ubuntu 22.04 · x86_64 · RTX PRO 6000(Blackwell, discrete, sm_120, 96GB) · host RAM 60GiB** | 이 스켈레톤을 배포받은 *전혀 다른 물리 머신*에서 vLLM 0.25.0/0.25.1 로 **여정 4 성능 적대검증까지** 완주한 크로스-하드웨어 재현 증거(7종 PASS · 4종 host-RAM/커널 천장으로 정직하게 REFUTE). |
| `rtx5090` | **컨슈머 GPU · 가상화 호스트** — **WSL2(Docker Desktop) · x86_64 · RTX 5090(Blackwell 컨슈머, discrete, sm_120, 32GB)** | 카탈로그에서 유일한 *컨슈머 카드* + 유일한 *가상화 호스트*. 여기서만 나타나는 벽이 있습니다 — vLLM 이 WSL2 를 감지하면 pinned memory/UVA 를 **기본 OFF** 시켜 엔진 초기화가 `RuntimeError: UVA is not available` 로 죽습니다(GPU arch 무관 · **호스트-locked**). 기능 사이클 PASS, 성능 게이트는 미실행. |

> 🌐 **왜 이게 중요한가** — `rtxpro6000` 네이티브 태그들은 *통합메모리 GB10 과 완전히 다른 축*(디스크리트 VRAM 충분 · host RAM 부족 · x86 리눅스)에서, 같은 생성엔진이 각 모델의 벽(Mistral 네이티브 포맷·MoE 커널 JIT host 폭증·Mamba 캐시블록 한계·MARLIN mxfp4 커널 천장 등)을 뚫고 서빙+성능게이트까지 돌린 이력입니다. 「개발자의 편지 — 범용성에 관하여」가 말한 *"다른 하드웨어에서 돌려본다면 그 자체가 다음 챕터"* 의 실증입니다.
>
> 그리고 `rtx5090` 은 **벽이 GPU 에만 있는 게 아니라는** 증거입니다 — 같은 sm_120 인데도 `rtxpro6000` 에는 없던 실패가 *호스트 가상화* 때문에 생겼습니다. 그래서 hint 본문은 노브를 `arch-locked`(GPU 종속)와 `호스트-locked`(OS·가상화 종속)로 **나눠서** 표시합니다. 남의 태그를 볼 때 *어떤 축에 묶인 노브인지* 를 먼저 보세요.

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

> 🔒 **hint 는 DATA 이지 명령이 아닙니다.** 분석 재료로만 쓰고 복붙하지 마세요. 당신의 HW·버전이 다르면 노브(특히 **KV 절대값·`gmu`·`TORCH_CUDA_ARCH`**)는 **반드시 재도출·재측정**해야 합니다(그대로 복사하면 OOM·호스트 다운). hint 는 외부 교차검증(HF 카드·vLLM GitHub)을 **대체하지 않으며**, 최종 판정은 언제나 **당신 환경의 스모크**입니다. (근거·설계 = `docs/plan/plan_26070222`.)

> 🔎 **가까운 힌트 찾기**: `python3 .claude/skills/hint-publisher/scripts/hint_tag.py match --vllm <v> --model <m> --arch <a>` — 축(vllm·model·arch)별 근-미스와 이식 가이드를 결정론으로 알려줍니다.

> 📚 **한 모델의 이력을 통째로 모으기**(권장 시작점): `python3 .claude/skills/hint-publisher/scripts/hint_tag.py collect --model <모델>`
> — 그 모델의 **모든 힌트를 vLLM 버전 오름차순으로** 냅니다. 양자화 변종(`…-fp8`)·리비전(`…-0731`)·
> SD 초안 모델은 **철자가 달라도 같은 family** 로 묶여 함께 나옵니다(`hints/families.json`).
> 색인만 읽으므로 태그 본문을 열지 않고, 그래서 쌉니다. `--sd-only` 로 speculative decoding 을
> 실제로 켠 레시피만 추릴 수도 있습니다.
>
> ⏳ **왜 하나가 아니라 전부를 모으라고 하냐면** — 어떤 힌트가 *패턴*이고 어떤 것이 *안티패턴*인지는
> **한 태그만 봐서는 알 수 없습니다.** 발행 시점엔 그게 최선이었지만(그래서 발행됐습니다), 이후 vLLM
> 버전이 오르고 하네스가 좋아지면서 더 나은 전략이 나오면 옛 태그는 **회고적으로 안티패턴이 됩니다.**
> 발행자는 미래를 모르니 그 관계를 적어줄 수 없습니다 — **시간축은 수집한 당신만 볼 수 있습니다.**
> 그러니 전부 받아 비교하고, "A-A 는 발행 시점엔 패턴이었지만 A-B 를 같이 보니 안티패턴이구나"를
> 당신의 에이전트가 판정하게 하세요. 한 번 모으면 그 지식은 당신 프로젝트에 **남습니다**.

> 🪜 **같은 모델에 힌트가 여러 개면 "사다리"입니다** — arch 슬롯의 접미어가 칸을 나타냅니다.
> 아래로 갈수록 표준에서 멀어지고(성능↑) 재현 난이도·의존이 커집니다. **낮은 칸부터** 올라가세요.
>
> | 칸 | 태그 | 스택 | decode |
> |---|---|---|---|
> | 1 노멀 | `hint/0.26.0/deepseek-v4-flash-0731/gb10x2` | stock 0.26.0 · 128K · spec ✗ | 19.65 t/s |
> | 2 컨텍스트 | `…/gb10x2-1m` | stock 0.26.0 · **1M** · spec ✗ | 18.36 t/s |
> | 3 변종 | `hint/0.26.1/…/gb10x2-dspark-1m` | **포크 핀**(jasl PR#41834) · 1M · **DSpark** | **31.01 t/s** |
>
> ⚠ **1칸 태그 본문에 사실오류 3건이 있습니다**(기존 태그는 재작성하지 않는 방침 — 정정은 2·3칸 §0):
> ① "fp8 단일" → 실제 routed experts 는 **MXFP4**(전체의 89%) ② "stock 에 dspark 없다" → **있으나
> 커널이 없어 못 쓴다** ③ "기대 13.20" → 그건 **PASS 문턱**(`floor_tps`)이고 기대치는 15.53.
> ③은 이 캠페인 태그 **4종 전부**에 있습니다 — 자기 측정치를 문턱과 비교하면 18% 후하게 자평하게 됩니다.

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
| `hint/0.24.0/gpt-oss-120b/gb10x2-sim-rtxpro6000x2` | 0.24.0 | gpt-oss-120b | gb10x2-sim-rtxpro6000x2 | multi 실 2노드 Ray TP2(main+sub GB10) — target=2xRTX PRO 6000 프로젝션 | active | docs/devlog/devlog_26070908_델타2-2_gpt-oss-120b_최종해결_및_4시나리오_전량PASS.md,docs/testlog/testlog_26070908_델타2-2_gpt-oss-120b_2xRTXPRO6000모사_실2노드_검증.md | 2026-07-09 | gpt-oss-120b(MXFP4 MoE, 60.77GiB) — 실 2노드 TP=2 Ray 분산서빙, 2xRTX PRO 6000(96GiB x2) 타겟 KV클램프 이식. 0.24.0 moe-backend=auto 회귀(TP=2 CompilationError) → marlin 전환 + gmu 0.8. |
| `hint/0.24.0/hy3/gb10` | 0.24.0 | hy3 | gb10 | multi 2노드 TP2 (Ray·RoCE) | active | hint/0.24.0/deepseek-v4-flash/gb10 | 2026-07-12 | Tencent Hy3-295B(21B active + 3.8B MTP) NVFP4-W4A16 를 2×GB10 에이전트-레디 서빙 — MARLIN NvFp4(W4A16 강제·cutlass/flashinfer 거부) + MTP spec-1 + fp8 KV 절대클램프 + enforce-eager + no-autotune + :opensource 파서패치. master(Ray head+API 동거)가 타이트 → memwatch 바닥 완화 필요. |
| `hint/0.24.0/qwen3.6-35b-a3b/gb10-sim-h200x2` | 0.24.0 | qwen3.6-35b-a3b | gb10-sim-h200x2 | multi 2노드 TP2 타겟(측정=1노드 GB10, target=H200x2 프로젝션) | active | docs/devlog/devlog_26070907_델타2-1_Qwen3.6-35B-A3B_최종해결_E2E_PASS.md,docs/testlog/testlog_26070820_델타2-1_Qwen3.6-35B-A3B_H200x2_재시도_FAIL.md | 2026-07-09 | Qwen3.6-35B-A3B(GDN 하이브리드 MoE, 66.97GiB) — H200x2(282GiB) 타겟 KV클램프 이식. GDN 커널 JIT 병렬폭주(MAX_JOBS=4로 해소) + gmu 0.85→0.75(추론-시점 잔여 JIT 대응). |
| `hint/0.25.0/mistral-small-4-119b/rtxpro6000` | 0.25.0 | mistral-small-4-119b | rtxpro6000 | single 1노드 | active | testlog_2026071620_1,testlog_2026071621_1 | 2026-07-19 | Mistral-Small-4-119B-2603-NVFP4(Pixtral MoE+MLA·A6B) 를 단일 RTX PRO 6000 서빙 — Mistral 네이티브 포맷(--*-format mistral·tekken) + TRITON_MLA + host RAM 60GiB 제약(MAX_JOBS=4·KV클램프·enforce-eager). 256K 기능 PASS·성능 host-constrained REFUTE. |
| `hint/0.25.0/nemotron-3-super-120b-a12b-nvfp4/rtxpro6000` | 0.25.0 | nemotron-3-super-120b-a12b-nvfp4 | rtxpro6000 | single 1노드 | active | testlog_26071617,testlog_2026071619_1 | 2026-07-19 | NemotronH LatentMoE(Mamba2+MoE+Attn·A12B) NVFP4 를 단일 RTX PRO 6000(96GiB·host RAM 60GiB) 서빙 — 커널 JIT host RAM 폭증(MAX_JOBS=4) + KV 절대클램프(프로파일링 skip) + enforce-eager. 256K PASS(기능)·성능 host-constrained REFUTE. |
| `hint/0.25.1/deepseek-v4-flash/gb10` | 0.25.1 | deepseek-v4-flash | gb10 | multi 2노드 TP2 (Ray·RoCE) | active | hint/0.24.0/deepseek-v4-flash/gb10 | 2026-07-16 | stock vLLM 0.25.1 = DSpark GB10 arch-wall(flashinfer decode_dsv4 page_block64 vs C128A) → jasl SM12x 포크 변종(b5c0d43b) + MATMUL_DECODE 성능레버로 DeepSeek-V4-Flash-DSpark 를 2×GB10 서빙(36.2 t/s · verdict PASS) |
| `hint/0.25.1/exaone-4.5-33b/rtxpro6000` | 0.25.1 | exaone-4.5-33b | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | LGAI-EXAONE/EXAONE-4.5-33B (Exaone4_5 dense VLM, SWA 하이브리드 64층=16 full+48 sliding(window 4096), bf16 ~64GiB) — RTX PRO 6000(96GiB discrete sm_120) TP1 최대-context(256K) M=24.1 t/s · verdict PASS(92.5% MBU) |
| `hint/0.25.1/gemma-4-26b-a4b/rtxpro6000` | 0.25.1 | gemma-4-26b-a4b | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | google/gemma-4-26B-A4B-it (Gemma4 MoE VLM SWA 하이브리드 30층=5 full+25 sliding, 128exp, bf16 ~49GiB, active 4B) — RTX PRO 6000 TP1 최대-context(256K) M=150.3 t/s · verdict PASS |
| `hint/0.25.1/gemma-4-31b/rtxpro6000` | 0.25.1 | gemma-4-31b | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | google/gemma-4-31B (Gemma4 dense VLM 31B, bf16 ~65GiB) — RTX PRO 6000 TP1 최대-context(256K) M=25.1 t/s · verdict PASS |
| `hint/0.25.1/gemma-4-e2b-it/rtx5090` | 0.25.1 | gemma-4-e2b-it | rtx5090 | single 1노드 | active | - | 2026-07-20 | vLLM 0.25.1 NGC26.05 source-build, RTX5090(sm_120) 16GiB예산 fp8 가중치+KV, max-model-len 131072(모델네이티브최대) batch1 최대-context 전략, WSL2 pin-memory 게이트 대응 |
| `hint/0.25.1/gpt-oss-120b/rtxpro6000` | 0.25.1 | gpt-oss-120b | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | openai/gpt-oss-120b (GptOss MoE 128exp/4tok, MXFP4, SWA sliding_window 128, ~66GiB, harmony reasoning, text-only) — RTX PRO 6000 TP1 128K(native 상한) M=62.6 t/s · verdict REFUTE(MARLIN mxfp4 커널 천장) |
| `hint/0.25.1/hyperclovax-think-32b/rtxpro6000` | 0.25.1 | hyperclovax-think-32b | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | naver-hyperclovax/HyperCLOVAX-SEED-Think-32B (dense reasoning 32B, bf16 ~66GiB) — RTX PRO 6000 TP1 M=24.0 t/s · verdict PASS |
| `hint/0.25.1/laguna-s-2.1/gb10` | 0.25.1 | laguna-s-2.1 | gb10 | single | active | `hint/0.25.1/deepseek-v4-flash/gb10`(동일 vLLM·arch, 다른 모델 — MoE 백엔드 정답이 정반대라 대조 사례로 유용) | 2026-07-22 | Laguna-S-2.1-NVFP4(117.6B-A8B MoE, poolside) 단일 GB10 서빙 · DFlash speculative decoding 유무 양노드 대조 검증 |
| `hint/0.25.1/minicpm5-1b-base/rtxpro6000` | 0.25.1 | minicpm5-1b-base | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | MiniCPM5-1B-Base (dense 1B, bf16 ~2GiB) — RTX PRO 6000(96GiB discrete sm_120) TP1 서빙 M=467 t/s · verdict PASS(벽 없음) |
| `hint/0.25.1/qwen3.5-122b-a10b-nvfp4/rtxpro6000` | 0.25.1 | qwen3.5-122b-a10b-nvfp4 | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | nvidia/Qwen3.5-122B-A10B-NVFP4 (Qwen3.5 MoE VLM 하이브리드, NVFP4 W4A4, 가중치 ~78GiB, active 10B) — RTX PRO 6000(96GiB, host RAM 60GiB) TP1 최대-context(256K) M=24.1 t/s · verdict REFUTE(host-constrained·수용) |
| `hint/0.25.1/qwen3.6-27b/rtxpro6000` | 0.25.1 | qwen3.6-27b | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | Qwen/Qwen3.6-27B (Qwen3.5 dense VLM 하이브리드 48층=12 full+36 linear, bf16 ~52GiB) — RTX PRO 6000 TP1 최대-context(256K) M=29.6 t/s · verdict PASS(92% MBU) |
| `hint/0.25.1/qwen3.6-35b-a3b/rtxpro6000` | 0.25.1 | qwen3.6-35b-a3b | rtxpro6000 | single 1노드 TP1 | active | — | 2026-07-19 | Qwen/Qwen3.6-35B-A3B (Qwen3.5 MoE VLM 하이브리드 40층=10 full+30 linear, bf16 ~67GiB, active 3B) — RTX PRO 6000 TP1 최대-context(256K) M=181.5 t/s · verdict PASS(79% MBU) |
| `hint/0.26.0/ax-4.0/gb10x2` | 0.26.0 | ax-4.0 | gb10x2 | multi 2노드 TP2 (Ray·RoCE) | active | testlog_26080201 | 2026-08-02 | SKT A.X-4.0(72B급 dense bf16, Qwen2ForCausalLM, 80층 GQA 64/8, native 131072) 2×GB10 TP=2. 가중치 133.9GiB → 노드당 67GiB. ★KV 가 GQA 공식과 정확히 일치한 사례 — 예측 3.2 vs 실측 3.20x, 104,858 vs 104,848 tok(하이브리드 모델과 대비된다). KV 16GiB 절대클램프 @32768. full 벤치 PASS 3.26 t/s(기대 1.13) — dense 대형이라 절대값이 낮고 루프라인 자체가 1.33 t/s 다. |
| `hint/0.26.0/deepseek-v4-flash-0731/gb10x2` | 0.26.0 | deepseek-v4-flash-0731 | gb10x2 | multi 2노드 TP2 (Ray·RoCE) | active | testlog_26080207 | 2026-08-02 | DeepSeek-V4-Flash-0731(정식판, fp8 block-quant e4m3/ue8m0, MoE routed256+shared1/active6, MLA+DSA index_topk 512, MTP 1층, 43층, 1M ctx) 2×GB10 TP=2. ★사다리 1칸=노멀(stock 0.26.0, 포크·패치 0건). ★DSpark arch-wall 가설 반증 — 0.25.1 시절 jasl fork 가 필요했으나 0.26.0 은 흡수했다. ★reasoning 분리는 파서/템플릿이 아니라 thinking 기본 False 가 원인. 가중치 155.4GiB → 노드당 77.7. KV 311,093 tok @131K(26.3 KiB/tok). full 벤치 PASS 19.65 t/s(기대 13.20). Hermes agent-ready(FC+reasoning 분리 실증). |
| `hint/0.26.0/deepseek-v4-flash-0731/gb10x2-1m` | 0.26.0 | deepseek-v4-flash-0731 | gb10x2-1m | multi 2노드 TP2 (Ray·RoCE) | active | testlog_26080214, plan_26080213 | 2026-08-02 | DeepSeek-V4-Flash-0731 @ **1M 컨텍스트** — 사다리 2칸. 1칸(128K) 대비 **바꾼 변수는 max-model-len 하나**. KV 절대클램프를 **한 바이트도 올리지 않고** 1,226,232 tok(6.85 KiB/tok · 동시성 1.17x) 확보 · decode **18.36 t/s**(1칸 19.65 대비 **−6.6%**) · verdict PASS(기대 15.53). Hermes 3종(completion·reasoning 분리·tool_call) 유지. ★ **KV bytes/token 은 max-model-len 의존**이 이 체크포인트에서 재확인됐다(27.0 KiB@128K → 6.85 KiB@1M). ★ 컨텍스트의 진짜 비용은 KV 가 아니라 **non-KV 오버헤드**(실측 바닥 −4.9 GiB). |
| `hint/0.26.0/glm-47-flash/gb10` | 0.26.0 | glm-47-flash | gb10 | single 1노드 | active | testlog_26080107 | 2026-08-01 | GLM-4.7-Flash(`zai-org/GLM-4.7-Flash`) bf16 MoE 64e · **MLA** · single tp=1 · KV 52.9 KiB/token(GQA 공식의 1/7) · full 벤치 PASS(MBU 85.7%) |
| `hint/0.26.0/gpt-oss-120b/gb10-single` | 0.26.0 | gpt-oss-120b | gb10-single | single 1노드 | active | testlog_26080114 | 2026-08-01 | gpt-oss-120b MXFP4 MoE 128e · single tp=1 · **auto 백엔드 기동실패 → HUMMING 필수** · 컨텍스트 민감도 3.8배 · 성능 **REFUTE** |
| `hint/0.26.0/laguna-s-2.1-fp8/gb10x2` | 0.26.0 | laguna-s-2.1-fp8 | gb10x2 | multi 2노드 TP2 (Ray·RoCE) | active | testlog_26080201 | 2026-08-02 | Laguna-S-2.1-FP8(poolside, compressed-tensors FP8, MoE 256e/10a, 48층 GQA 8/128, native 262144) 2×GB10 TP=2. 가중치 113.2GiB → 노드당 56.6GiB. KV 22GiB 절대클램프 · moe-backend triton · disable-custom-all-reduce(RoCE) · poolside_v1 파서(FC+reasoning). full 벤치 PASS 28.16 t/s(기대 13.03) — 이번 캠페인 4종 중 최고. |
| `hint/0.26.0/lfm2-8b-a1b/gb10` | 0.26.0 | lfm2-8b-a1b | gb10 | single 1노드 | active | testlog_26073121 | 2026-07-31 | LFM2-8B-A1B bf16 MoE 32e 하이브리드(conv18+attn6) · single tp=1 · KV 12KiB/token(전층가정의 1/4) · cold JIT 23초 · full 벤치 PASS(MBU 71%) |
| `hint/0.26.0/ministral-3-8b/gb10` | 0.26.0 | ministral-3-8b | gb10 | single 1노드 | active | testlog_26073115,testlog_26073116,testlog_26073117 | 2026-07-31 | Ministral-3-8B-Instruct-2512 FP8 vision-멀티모달 · single tp=1 · KV 절대클램프 20GiB · full 벤치 PASS(MBU 94.4%) |
| `hint/0.26.0/olmo-3.1-32b/gb10` | 0.26.0 | olmo-3.1-32b | gb10 | single 1노드 | active | testlog_26080110 | 2026-08-01 | Olmo-3.1-32B-Instruct(`allenai/Olmo-3.1-32B-Instruct`) bf16 dense · 하이브리드 sliding48+full16 · KV 88.0 KiB/token(**sliding = 윈도우×2**) · full 벤치 PASS(MBU 87.5%) |
| `hint/0.26.0/qwen35-122b-a10b-nvfp4/gb10x2` | 0.26.0 | qwen35-122b-a10b-nvfp4 | gb10x2 | multi 2노드 TP2 (Ray·RoCE) | active | testlog_26080118,testlog_26080201 | 2026-08-02 | Qwen3.5-122B-A10B-NVFP4(compressed-tensors NVFP4, MoE 256e/8a, 하이브리드 attn, vision) 2×GB10 TP=2. ★single 불가 — 로드 피크 114.8GiB(가중치버퍼 선할당 75.0 + 로드중 누적 31.8)가 gmu 0.90 예산 109.5GiB 를 넘는다. 정상상태 75.9GiB(예산 69%)만 보면 오판한다. TP=2 로 노드당 실측 35.75GiB. KV 12.4KiB/tok(676,697 tok @8GiB 절대클램프) — 매트릭스 추정 24KiB의 절반(하이브리드라 GQA 공식 불성립). 동시성 2.58x@262144. full 벤치 PASS 22.59 t/s(기대 12.27). |
| `hint/0.26.1/deepseek-v4-flash-0731/gb10x2-dspark-1m` | 0.26.1 | deepseek-v4-flash-0731 | gb10x2-dspark-1m | multi 2노드 TP2 (Ray·RoCE) | active | testlog_26080217, testlog_26080215, plan_26080213 | 2026-08-02 | DeepSeek-V4-Flash-0731 @ **1M 컨텍스트 + DSpark speculative decoding** — 변종 트랙(`jasl/vllm` PR#41834 SHA 핀 소스빌드). decode **31.01 t/s**(같은 하드웨어 stock 18.36 대비 **1.69배**) · accept_len **2.286** · KV 1,206,214 tok @1,048,576(클램프 8 GiB 불변) · Hermes 3종(completion·reasoning 분리·tool_call) 통과. ★ **stock vLLM 0.26.0 으로는 이 조합이 불가능하다** — spec 경로 2개가 서로 다른 이유로 막힌다. ★ 포크가 여는 것은 성능 노브가 아니라 **sm_120 커널 인스턴스 자체**다. |
| `hint/0.27.0/LFM2.5-2.6B/gb10` | 0.27.0 | LFM2.5-2.6B | gb10 | single 1노드 | active | — | 2026-08-11 | 16K 컨텍스트·gmu=0.4 예산 최대동시성(41), 서브 GB10 단독 서빙(노드당 2모델 동시서빙 설계 중 하나) |
| `hint/0.27.0/LFM2.5-2.6B/gb10-v2` | 0.27.0 | LFM2.5-2.6B | gb10-v2 | single 1노드 | active | hint/0.27.0/LFM2.5-2.6B/gb10 | 2026-08-12 | [정정판] reasoning_parser=qwen3 필수 — 구태그(hint/0.27.0/LFM2.5-2.6B/gb10)는 파서 미설정 결함 있음(폐기 아님, 교훈 보존) |
| `hint/0.27.0/Qwen3-4B/gb10` | 0.27.0 | Qwen3-4B | gb10 | single 1노드 | active | — | 2026-08-11 | 16K 컨텍스트·gmu=0.4 예산 최대동시성(4), 서브 GB10 단독 서빙, tool_call_parser=hermes(qwen3 계열 함정 주의) |
| `hint/0.27.0/deepseek-v4-flash-0731/gb10x2-dspark-1m` | 0.27.0 | deepseek-v4-flash-0731 | gb10x2-dspark-1m | multi 2노드 TP2 (Ray·RoCE) | active | hint/0.26.1/deepseek-v4-flash-0731/gb10x2-dspark-1m | 2026-08-15 | DeepSeek-V4-Flash-0731 @ **1M 컨텍스트 + DSpark speculative decoding** on vLLM **0.27.0** — ★ **포크를 핀하지 마라. 자체 이식(3+1+1 빌드패치)으로 같은 성능이 나온다.** decode **36.87 t/s**(자체이식) vs **37.05 t/s**(포크핀) = **동률**(+0.49% — 같은 이미지의 런간 분산 6.8~7.3% 의 **1/14**)이고, **배치에서는 자체이식이 두 쌍 독립으로 이긴다**(동시성 2 **+24.5% / +16.7%** · 동시성 4 **+12.6% / +24.5%**). 같은 하드웨어 stock 0.27.0(spec off) 18.92 대비 **약 1.95배**(⚠ R0 는 1회 측정 — 런간 분산 ~7% 를 감안해 읽어라). accept_len **2.625** · KV 1,206,214 tok @1,048,576(클램프 8 GiB 불변). ★ stock 0.27.0 으로는 이 조합이 **여전히 불가능하다** — spec 경로 2개가 **서로 다른 이유로** 막힌다(0.26.0 과 같은 벽이 살아 있다). |
| `hint/0.27.0/gemma-4-E2B-it/gb10` | 0.27.0 | gemma-4-E2B-it | gb10 | single 1노드 | active | — | 2026-08-11 | 16K 컨텍스트·gmu=0.4 예산 최대동시성(181), 메인 GB10 단독 서빙(노드당 2모델 동시서빙 설계 중 하나) |
| `hint/0.27.0/gemma-4-E4B-it/gb10` | 0.27.0 | gemma-4-E4B-it | gb10 | single 1노드 | active | — | 2026-08-11 | 16K 컨텍스트·gmu=0.4 예산 최대동시성(10), fp8 quant, 메인 GB10 단독 서빙(노드당 2모델 동시서빙 설계 중 하나) |
| `hint/0.27.0/gemma-4-e2b-it/gb10x2` | 0.27.0 | gemma-4-e2b-it | gb10x2 | multi 2노드 TP2 | active | — | 2026-08-20 | <!-- ⚠ 절 헤딩(`## 1.` ~ `## 7.`)을 **지우지 마라** — `hint_tag seal` 의 린터 L1 이 fail-closed 로 |
| `hint/0.27.0/gemma-4-e4b-it/gb10x2` | 0.27.0 | gemma-4-e4b-it | gb10x2 | multi 2노드 TP2 | active | — | 2026-08-20 | <!-- ⚠ 절 헤딩(`## 1.` ~ `## 7.`)을 **지우지 마라** — `hint_tag seal` 의 린터 L1 이 fail-closed 로 |
| `hint/0.27.0/hy3/gb10x2` | 0.27.0 | hy3 | gb10x2 | multi 2노드 TP2 | active | hint/0.24.0/hy3/gb10 | 2026-08-20 | object d564a8ae18b5abcc0b35a862e66bc8967155ddd1 |
| `hint/0.27.0/hy3/gb10x2-blk64` | 0.27.0 | hy3 | gb10x2-blk64 | multi 2노드 TP2 | active | hint/0.27.0/hy3/gb10x2 | 2026-08-20 | object d564a8ae18b5abcc0b35a862e66bc8967155ddd1 |
| `hint/0.27.0/hy3/gb10x2-cudagraph` | 0.27.0 | hy3 | gb10x2-cudagraph | multi 2노드 TP2 | active | hint/0.27.0/hy3/gb10x2 | 2026-08-20 | object d564a8ae18b5abcc0b35a862e66bc8967155ddd1 |
| `hint/0.27.0/hy3/gb10x2-nospec` | 0.27.0 | hy3 | gb10x2-nospec | multi 2노드 TP2 | active | hint/0.27.0/hy3/gb10x2 | 2026-08-20 | object d564a8ae18b5abcc0b35a862e66bc8967155ddd1 |
| `hint/0.27.0/hy3/gb10x2-spec1` | 0.27.0 | hy3 | gb10x2-spec1 | multi 2노드 TP2 | active | hint/0.27.0/hy3/gb10x2 | 2026-08-20 | object d564a8ae18b5abcc0b35a862e66bc8967155ddd1 |
| `hint/0.27.0/hy3/gb10x2-spec3` | 0.27.0 | hy3 | gb10x2-spec3 | multi 2노드 TP2 | active | hint/0.27.0/hy3/gb10x2 | 2026-08-20 | object d564a8ae18b5abcc0b35a862e66bc8967155ddd1 |
| `hint/0.27.0/hy3/gb10x2-tritonattn` | 0.27.0 | hy3 | gb10x2-tritonattn | multi 2노드 TP2 | active | hint/0.27.0/hy3/gb10x2 | 2026-08-20 | object d564a8ae18b5abcc0b35a862e66bc8967155ddd1 |
| `hint/0.27.0/lfm2.5-2.6b/gb10x2` | 0.27.0 | lfm2.5-2.6b | gb10x2 | multi 2노드 TP2 | active | — | 2026-08-20 | <!-- ⚠ 절 헤딩(`## 1.` ~ `## 7.`)을 **지우지 마라** — `hint_tag seal` 의 린터 L1 이 fail-closed 로 |
| `hint/0.27.0/qwen3-4b/gb10x2` | 0.27.0 | qwen3-4b | gb10x2 | multi 2노드 TP2 | active | — | 2026-08-20 | <!-- ⚠ 절 헤딩(`## 1.` ~ `## 7.`)을 **지우지 마라** — `hint_tag seal` 의 린터 L1 이 fail-closed 로 |
| `hint/0.27.1.dev0/qwen3.8-27b/gb10-1m-fp8kv-fp8w` | 0.27.1.dev0 | qwen3.8-27b | gb10-1m-fp8kv-fp8w | single 1노드 | active | — | 2026-08-24 | Qwen3.8-27B 를 GB10 단일노드에서 **1M 컨텍스트**로 띄운 레시피 — fp8 KV 가 티어를 열고, 온-더-플라이 가중치 fp8 이 속도를 1.73× 올린다. |
| `hint/0.27.1.dev0/qwen3.8-27b/gb10-262k-stock` | 0.27.1.dev0 | qwen3.8-27b | gb10-262k-stock | single 1노드 | active | — | 2026-08-24 | Qwen3.8-27B 를 GB10 단일노드에서 **네이티브 262,144 컨텍스트**로 띄운 기준선 레시피 — rope 주입 없이, MTP n=3 만으로 기준선 대비 2.18×. |
| `hint/0.27.1.dev0/qwen3.8-27b/gb10-524k-stock` | 0.27.1.dev0 | qwen3.8-27b | gb10-524k-stock | single 1노드 | active | — | 2026-08-24 | Qwen3.8-27B 를 GB10 단일노드에서 **512K(524,288) 컨텍스트 · bf16 KV** 로 띄운 레시피 — 가중치를 bf16 로 두고도 서는 가장 높은 티어. |
| `hint/0.27.1.dev0/qwen3.8-27b/gb10x2-1m-tp2` | 0.27.1.dev0 | qwen3.8-27b | gb10x2-1m-tp2 | multi 2노드 TP2 (Ray·RoCE) | active | — | 2026-08-24 | Qwen3.8-27B 를 2×GB10 Ray TP=2 + **1M 컨텍스트**(rope YaRN factor=4.0 · fp8 KV · fp8 가중치 양자화)로 서빙 — **THERMAL_LIMIT_LITE_ONLY**: warm + full bench 가 GB10 SoC 95℃ 초과 패턴으로 docker_kill(피크 97.9℃ · M-1 동형). lite cold 단일 측정만(TTFT 218 ms · output 14.54 t/s · KV 1,140,405 tokens). 캠페인 승자는 형제 single R7 1M-fp8kv-fp8w 18.69 t/s(full bench). |
| `hint/0.27.1.dev0/qwen3.8-27b/gb10x2-262k-tp2` | 0.27.1.dev0 | qwen3.8-27b | gb10x2-262k-tp2 | multi 2노드 TP2 (Ray·RoCE) | active | — | 2026-08-24 | Qwen3.8-27B 를 2×GB10 Ray TP=2 + 262k 컨텍스트로 서빙 — THERMAL_LIMIT_LITE_ONLY: warm+full 벤치가 SoC 95℃ 초과로 docker_kill. lite cold 단일 측정만(TTFT 311ms · 8.40 t/s). 승자는 형제 single R7 1M-fp8kv-fp8w 18.69 t/s. |
| `hint/0.27.1/deepseek-v4-flash-0731/gb10x2-dspark-1m` | 0.27.1 | deepseek-v4-flash-0731 | gb10x2-dspark-1m | multi 2노드 TP2 (Ray·RoCE) | active | hint/0.27.0/deepseek-v4-flash-0731/gb10x2-dspark-1m | 2026-08-21 | DeepSeek-V4-Flash-0731 @ **1M 컨텍스트 + DSpark speculative decoding** on vLLM **0.27.1** — ★ **선행 0.27.0 자체이식본을 그대로 얹지 마라. 조용히 되돌아간다.** 0.27.1 의 유일한 런타임 델타 #50424(`qwen3_dspark.py` quantized DSpark Markov head, **+4줄**)를 0.27.0 이식본이 **덮어서 없앤다** — 에러도 경고도 없다. 좌표를 옮겨 **재파생**하면 108파일 중 바이트가 바뀌는 것은 그 1파일뿐이다. decode **37.36 t/s**(@동시성1) · verdict **PASS**(c=30 기준 ratio 1.245) · accept_len **2.685** · KV **1,206,214 tok @1,048,576**(클램프 8 GiB 불변). ★ **0.27.0 대비 성능은 동등(noise-level)** — 이 bump 가 확인해야 했던 것은 속도 향상이 아니라 **회귀 부재**이고, 그것이 확인됐다. |
<!-- hint-index:rows -->
