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

<!-- ⚠ 아래 두 마커 사이는 **기계 생성 구역**이다(plan_26090107 D1.1). 손으로 고치지 마라 —
     `hint_catalog.py derive` 가 **원격 발행 태그**에서 통째로 다시 만든다. 진실원천은
     `git ls-remote --tags <remote> 'refs/tags/hint/*'` 이고 brief 는 태그 오브젝트에서 파싱한다.
     이전에는 여는 마커가 없어 관리 구역의 시작이 모호했고, 마커가 유실되면 전 행이
     조용히 사라질 수 있었다(감사 ⑬). -->
<!-- hint-index:rows -->
| 태그 | vLLM | 모델 | arch | recipe | topology | 상태 | 대체/관련 | 최종검증 | brief |
|---|---|---|---|---|---|---|---|---|---|
| `hint/0.18.0/gpt-oss-120b/gb10-multi` | 0.18.0 | gpt-oss-120b | gb10-multi | 0.18.0 prebuilt wheel + ray 로 gpt-oss-120b(MXFP4)를 GB10 2노드 TP=2 분산 서빙 — 53.92 t/s(단일노드 34.42 대비 **1.57배**) · 성능 PASS · **같은 하드웨어에서 0.19.1 은 분산이 성립하지 않는다**(음성 증거 동봉) · 이 태그의 archive 가 곧 재현 키트다 |
| `hint/0.18.0/gpt-oss-120b/gb10-single` | 0.18.0 | gpt-oss-120b | gb10-single | 0.18.0 prebuilt wheel 로 gpt-oss-120b(MXFP4)를 단일 GB10 에 최대 컨텍스트(131072)로 서빙 — 34.42 t/s(MBU 77.7%) · 성능 PASS · **서빙 벽 0개, 벽은 측정·판정 평면에 있었다** · 0.19.1 과 단일에서는 구분되지 않는다(버전 선택 근거는 분산에 있다) · 이 태그의 archive 가 곧 재현 키트다 |
| `hint/0.18.0/gpt-oss-20b/gb10` | 0.18.0 | gpt-oss-20b | gb10 | 0.18.0 prebuilt wheel(다운그레이드)로 gpt-oss-20b(MXFP4)를 단일 GB10에 최대 컨텍스트(131072)로 서빙 — TRITON_ATTN(유일 후보) · 46.51 t/s(ratio 2.37x expected) · 성능 PASS · **이 태그의 archive가 곧 페이로드다** |
| `hint/0.18.0/gpt-oss-20b/gb10-multi` | 0.18.0 | gpt-oss-20b | gb10-multi | 0.18.0 prebuilt wheel + ray 로 gpt-oss-20b(MXFP4)를 GB10 2노드 TP=2 분산 서빙 — 69.36 t/s(단일노드 48.58 대비 **1.43배**) · 성능 PASS · **작은 모델일수록 TP=2 이득이 작다**(헤드룸 부족) · 같은 하드웨어에서 0.19.1 은 분산 불가 · 이 태그의 archive 가 곧 재현 키트다 |
| `hint/0.19.1/gpt-oss-120b/gb10-single` | 0.19.1 | gpt-oss-120b | gb10-single | 0.19.1 prebuilt wheel 로 gpt-oss-120b(MXFP4)를 단일 GB10 에 서빙 — auto=TRITON_ATTN+MARLIN · 34.55 t/s(MBU 77.9%) · 성능 PASS · **이 태그의 archive 가 곧 페이로드다** |
<!-- hint-index:rows -->
