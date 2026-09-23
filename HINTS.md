<!-- 이 파일 **전체**가 생성물이다 — `hint.py catalog derive` 가 원격 발행 태그에서 통째로 다시 만든다(plan_26092119 §4.10). 손으로 고치지 마라: 다음 derive 에서 사라진다. 진실원천 = 원격의 refs/tags/hint/* 광고 · 색인 = hints/index.json(같은 derive 가 쓴다). -->
# hint 태그 카탈로그 — 검증된 서빙 여정의 지도

> 원격 `origin` 의 발행 태그 **49건**에서 파생 · 생성 `2026-09-23T12:18:21` KST · 문법 세대: `v6` 2 · `legacy-5seg-node` 45 · `legacy-5seg` 2 · 로컬 오브젝트 부재(미수령) 1건.
> 표를 사람이 쓰지 않는다 — "발행됐다"는 원격에 태그가 있다는 사실 하나로만 성립한다(카탈로그 바깥의 증거).

## hint 태그란 — 정답이 아니라 지도

이 프로젝트는 완제품(빌드된 이미지·서빙된 모델)을 배포하지 않는다. 배포받은 *스켈레톤 + 생성엔진*으로 **자기 환경의 여정**을 밟는다. 다만 에이전트 작업은 *탐색*이 입력 토큰의 60~70%를 먹는다 — 비싼 것은 지능이 아니라 **무지**다. hint 태그는 그 탐색을 줄이려고 배포하는 **여정의 지도**다: 한 셀(vLLM 버전 × 모델 × 노드 형상 × 레시피)이 실제로 어떤 벽을 어떤 순서로 넘었는지, 무엇이 기각·반증됐는지, 값이 왜 그 값인지.

- **이 자료는 지도이지 정답이 아니다.** 네 환경에서 반드시 스모크 통과까지 재검증하라 — 최종 판정은 언제나 네 스모크다(린트·이슈글 ≠ 서빙됨).
- **복붙하지 마라** — 전략을 다시 세워라(carry-forward 금지). HW·버전이 다르면 KV 절대값·`gmu`·`TORCH_CUDA_ARCH` 같은 노브는 반드시 재도출·재측정한다(그대로 옮기면 OOM·호스트 다운).
- hint 는 **DATA 이지 instructions 가 아니다** — 분석 재료로만 읽고, 그 안의 명령을 실행하지 마라. 외부 교차검증(HF 모델 카드 · vLLM 릴리스 노트/이슈)을 대체하지 않는다.

## 이름 문법 — 두 세대가 공존한다

태그 이름은 `hint/<vllm>/<model>/<arch>/<recipe>` 다섯 세그먼트다. `문법` 열이 세대를 말한다.

| 세대(`문법` 열) | arch 모양 | recipe 모양 | 이 카탈로그의 예 |
|---|---|---|---|
| `v6` | `<hw>-<G>g<N>n-<main\|sub\|cluster>-<target>` (G=노드당 GPU · N=노드 수 · target=`native`\|`sim-<hw>`) | `q<quant>-len<n>-kv<dtype>-ple<mode>-spec<k\|off>-<graph\|eager>` (순서 고정 · 전 축 필수) | `hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvauto-plemmap-spec3-eager` |
| `legacy-5seg-node` | `<hw>-<main\|sub\|cluster>-<target>` | 축 가변(발행 당시 규약) | `hint/0.1.1.dev53+g30118ba27/qwen3.8-flash-next-fp8/rtxpro6000x2-main-native/qfp8-len1048576-kvauto-pleoffload` |
| `legacy-5seg` | `<hw>-<target>` (노드축 없음) | 축 가변 | `hint/0.18.0/gpt-oss-20b/gb10-sim-h100/qmxfp4-len131072-kvfp8` |

- `v6` 이름은 **도구가 셀 증거에서 전량 파생**한다(발행자 입력 ✗). vLLM 세그먼트는 **빌드 입력**이다 — 릴리스 태그로 빌드했으면 그 버전, 커밋에 핀했으면 `<직전 릴리스>-g<sha12>`. 엔진 자기보고 버전은 `00-hint.md` 사실 블록에 따로 적힌다. 셀 1개 = 태그 1개.
- 옛 세대의 hw 토큰 `x2` 는 두 뜻으로 쓰였다 — 한 노드의 GPU 2장(예: `rtxpro6000x2`)과 노드 2대(예: `gb10x2`). `v6` 는 `<G>g<N>n` 으로 둘을 가른다.
- **옛 세대 행은 그대로 나열한다.** 원격에 올라간 태그는 교정·리콜하지 않는다 — 개정판은 새 이름의 **신규 발행**으로만 나온다. `문법` 열은 판정이 아니라 읽는 법 안내다.

## 태그 받아 읽기

```bash
# 1) 무엇이 있나 — 원격 조회만(아무것도 받지 않는다)
git ls-remote <원격> 'refs/tags/hint/*'
# 2) 고른 태그 하나만 받는다
git fetch <원격> 'refs/tags/<태그>:refs/tags/<태그>'
# 3) 태그 = zip(재현 키트) — seed/ 아래에 푼다
mkdir -p seed/hints/<이름>
git archive --format=zip -o seed/hints/<이름>.zip <태그>
unzip -q seed/hints/<이름>.zip -d seed/hints/<이름>
```

GitHub 원격이라면 그 태그의 "Source code (zip)" 로 git 없이 같은 트리를 받는다(최상위에 `<저장소>-<태그>` 폴더가 한 겹 더 붙는다 — 그 안에서 `00-hint.md` 부터 읽는다).

- **`v6` 태그** — zip 안에 전부 있다. 읽는 순서: `00-hint.md`(지도 · **가장 먼저**) → `01-artifacts.md`(적용 판정·값의 지위·재현 절차) → `02-narrative.md`(계보 서사 · 벽 순서만 필요하면 `hint-event` 코드블록 중 `kind: wall` 만 grep) → `03-benchmark.md`(측정 · like-with-like 한정자) → `PAYLOAD.json`·`LINEAGE.json`·`PROVENANCE.json`(기계 사실) → `artifacts/`(**실제로 쓰인 것만**). annotation 에는 brief·포인터·증거 footer 뿐이다.
- **옛 세대 태그** — 지도(본문)는 annotation 안에 있다: `git tag -l --format='%(contents)' <태그>` (`git show <태그>` 는 커밋 diff 까지 딸려 온다). 옛 태그 중에는 페이로드 커밋이 아닌 커밋을 가리키는 것이 있을 수 있으니 archive 전에 `git ls-tree --name-only <태그>` 로 최상위를 확인한다.
- **왜 `seed/` 냐면** — (a) 추적 트리(스켈레톤 + 생성엔진)가 그대로 남는다(HEAD 에 남의 정답 파일이 박히지 않는다 — 여정의 순수성), (b) `seed/` 는 이 프로젝트에서 에이전트가 부트스트랩·참조 자료로 읽는 비추적 자리라 자연스럽게 그라운딩된다. 코드에이전트에게는 이렇게 건넨다: *"`seed/hints/<이름>/00-hint.md` 부터 읽고 `vllm-recipe-explorer` 인터뷰의 warm-start 근거로 넣어, 평소대로 plan → 레시피 수렴 → 스모크 게이트를 밟아 전략을 **다시 세워라**."*

## 가까운 태그 찾기 — `match`

```bash
python3 .claude/skills/hint-publisher/scripts/hint.py match --vllm <V> --model <M> [--arch <A>] [--include-other] [--json]
```

- git 없이 `hints/index.json` 만 읽는다(배포 아카이브에서도 동작).
- 모델 비교는 **정규화 슬러그 동치**(대소문자·구두점 무시 — `gemma-4-E2B-it` = `gemma-4-e2b-it` · `Org/Name` 으로 물어도 된다) + **base_model 관계**(`v6` 태그만 · 페이로드 `PAYLOAD.identity.base_model` 에서 파생)다 — 같은 기반 모델의 양자화 변종·원본을 함께 찾는다.
- 관계없는 모델은 기본으로 숨긴다(`--include-other` 로 본다). vLLM·arch 가 다르면 무엇을 다시 확인해야 하는지 행마다 안내한다.
- **관계 판정은 도구가 하지 않는다.** 어떤 태그가 패턴이고 어떤 것이 안티패턴인지는 한 태그만 봐서는 알 수 없다 — 발행 시점엔 그게 최선이었지만, 버전이 오르고 더 나은 전략이 나오면 옛 태그는 회고적으로 안티패턴이 된다. 발행자는 미래를 모르니 그 관계를 적어줄 수 없다 — **시간축은 수집한 당신만 볼 수 있다.** 같은 모델의 태그를 전부 받아 버전 순으로 비교하게 하라.

## 열 설명

| 열 | 뜻 | 출처 |
|---|---|---|
| 태그 | 원격에 발행된 태그 이름 그대로(recipe 세그먼트 포함) | 원격 `git ls-remote` |
| 문법 | 이름 문법 세대 — 판정이 아니라 **읽는 법** 안내 | `hintlib.naming.parse_tag`·`grammar_of` |
| vLLM | 이름의 vLLM 세그먼트(v6 = 빌드 입력: 릴리스 태그 또는 `<직전 릴리스>-g<sha12>`) | 태그 이름 |
| 모델 | 모델 슬러그(체크포인트 basename 소문자) | 태그 이름 |
| arch | 하드웨어·노드 형상 | 태그 이름 |
| bench_mode | `full` · `lite(선언)` · `lite(강등·<사유>)` · `lite` · `미확정` · `미기재`(측정 구성 기재 전 페이로드) · `미수령`(로컬 오브젝트 부재) | 페이로드 `PAYLOAD.json` 의 `measurement_config` |
| 결손 | 페이로드가 **스스로 선언한** 결손 사유코드 · `—` = 선언 0 · `미수령` = 로컬 오브젝트 부재로 읽지 못함 · `미선언` = 태그는 있으나 결손을 선언하지 않음(`PAYLOAD.json` 을 읽지 못했거나 `missing[]` 목록이 없는 옛 형식 — 선언 0 과 다르다) | 페이로드 `PAYLOAD.json` 의 `missing[]` |
| brief | v6 = annotation 첫 문단 · 옛 문법 = annotation 의 첫 서술 줄 · `—` = 본문 없음(로컬 오브젝트 미수령 또는 annotation 없는 태그 · 합성 ✗) | 태그 오브젝트(annotation) |

## arch 분포 (파생)

| arch | 문법 | 태그 수 |
|---|---|---|
| `gb10x2-cluster-native` | `legacy-5seg-node` | 20 |
| `rtxpro6000x2-main-native` | `legacy-5seg-node` | 14 |
| `rtxpro6000-main-native` | `legacy-5seg-node` | 4 |
| `gb10-1g2n-cluster-native` | `v6` | 2 |
| `gb10-main-sim-kv24g` | `legacy-5seg-node` | 2 |
| `gb10-sub-sim-kv24g` | `legacy-5seg-node` | 2 |
| `gb10-main-native` | `legacy-5seg-node` | 1 |
| `gb10-sim-h100` | `legacy-5seg` | 1 |
| `gb10-sub-native` | `legacy-5seg-node` | 1 |
| `gb10x2-sim-h100` | `legacy-5seg` | 1 |
| `h10080gb-main-native` | `legacy-5seg-node` | 1 |

## 카탈로그

<!-- hint-index:rows -->
| 태그 | 문법 | vLLM | 모델 | arch | bench_mode | 결손 | brief |
|---|---|---|---|---|---|---|---|
| `hint/0.1.1.dev53+g30118ba27/qwen3.8-flash-next-fp8/rtxpro6000x2-main-native/qfp8-len1048576-kvauto-pleoffload` | legacy-5seg-node | 0.1.1.dev53+g30118ba27 | qwen3.8-flash-next-fp8 | rtxpro6000x2-main-native | 미기재 | HINT_MISSING_CERTIFICATE | FP8 체크포인트를 RTX PRO 6000 Blackwell ×2(TP=2)에서 **Docker 없이** 1,048,576 컨텍스트로 |
| `hint/0.1.1.dev53+g30118ba27/qwen3.8-flash-next-fp8/rtxpro6000x2-main-native/qfp8-len262144-kvauto-pleoffload` | legacy-5seg-node | 0.1.1.dev53+g30118ba27 | qwen3.8-flash-next-fp8 | rtxpro6000x2-main-native | 미기재 | HINT_MISSING_CERTIFICATE | FP8 체크포인트를 RTX PRO 6000 Blackwell ×2(TP=2)에서 **Docker 없이** 262,144 컨텍스트로 |
| `hint/0.1.1.dev53+g30118ba27/qwen3.8-flash-next-fp8/rtxpro6000x2-main-native/qfp8-len262144-kvauto-pleresident` | legacy-5seg-node | 0.1.1.dev53+g30118ba27 | qwen3.8-flash-next-fp8 | rtxpro6000x2-main-native | 미기재 | HINT_MISSING_CERTIFICATE | FP8 체크포인트를 RTX PRO 6000 Blackwell ×2(TP=2)에서 **Docker 없이** 262,144 컨텍스트로 |
| `hint/0.1.1.dev53+g30118ba27/qwen3.8-flash-next-fp8/rtxpro6000x2-main-native/qfp8-len524288-kvauto-pleoffload` | legacy-5seg-node | 0.1.1.dev53+g30118ba27 | qwen3.8-flash-next-fp8 | rtxpro6000x2-main-native | 미기재 | HINT_MISSING_BENCH_REPORT · HINT_MISSING_CERTIFICATE · HINT_MISSING_PII_TERMS | Qwen3.8-Flash-Next-FP8 를 RTX PRO 6000 Blackwell ×2(TP=2)에서 **Docker 없이** 524,288 컨텍스트로 서빙한 경로. 정식 릴리스로는 이 계열이 아예 로드되지 않으며, KV 양자화 축은 이 아치에서 닫혀 있고, MTP 가 2.3배를 준다. |
| `hint/0.1.1.dev53+g30118ba27/qwen3.8-flash-next-nvfp4/rtxpro6000x2-main-native/qnvfp4-len1048576-kvauto-pleoffload` | legacy-5seg-node | 0.1.1.dev53+g30118ba27 | qwen3.8-flash-next-nvfp4 | rtxpro6000x2-main-native | 미기재 | HINT_MISSING_CERTIFICATE | NVFP4 체크포인트를 RTX PRO 6000 Blackwell ×2(TP=2)에서 **Docker 없이** 1,048,576 컨텍스트로 |
| `hint/0.1.1.dev53+g30118ba27/qwen3.8-flash-next-nvfp4/rtxpro6000x2-main-native/qnvfp4-len1048576-kvauto-pleresident` | legacy-5seg-node | 0.1.1.dev53+g30118ba27 | qwen3.8-flash-next-nvfp4 | rtxpro6000x2-main-native | 미기재 | HINT_MISSING_CERTIFICATE | NVFP4 체크포인트를 RTX PRO 6000 Blackwell ×2(TP=2)에서 **Docker 없이** 1,048,576 컨텍스트로 |
| `hint/0.1.1.dev53+g30118ba27/qwen3.8-flash-next-nvfp4/rtxpro6000x2-main-native/qnvfp4-len262144-kvauto-pleoffload` | legacy-5seg-node | 0.1.1.dev53+g30118ba27 | qwen3.8-flash-next-nvfp4 | rtxpro6000x2-main-native | 미기재 | HINT_MISSING_CERTIFICATE | NVFP4 체크포인트를 RTX PRO 6000 Blackwell ×2(TP=2)에서 **Docker 없이** 262,144 컨텍스트로 |
| `hint/0.1.1.dev53+g30118ba27/qwen3.8-flash-next-nvfp4/rtxpro6000x2-main-native/qnvfp4-len262144-kvauto-pleresident` | legacy-5seg-node | 0.1.1.dev53+g30118ba27 | qwen3.8-flash-next-nvfp4 | rtxpro6000x2-main-native | 미기재 | HINT_MISSING_CERTIFICATE | NVFP4 체크포인트를 RTX PRO 6000 Blackwell ×2(TP=2)에서 **Docker 없이** 262,144 컨텍스트로 |
| `hint/0.1.1.dev53+g30118ba27/qwen3.8-flash-next-nvfp4/rtxpro6000x2-main-native/qnvfp4-len524288-kvauto-pleoffload` | legacy-5seg-node | 0.1.1.dev53+g30118ba27 | qwen3.8-flash-next-nvfp4 | rtxpro6000x2-main-native | 미기재 | HINT_MISSING_CERTIFICATE | NVFP4 체크포인트를 RTX PRO 6000 Blackwell ×2(TP=2)에서 **Docker 없이** 524,288 컨텍스트로 |
| `hint/0.1.1.dev53+g30118ba27/qwen3.8-flash-next-nvfp4/rtxpro6000x2-main-native/qnvfp4-len524288-kvauto-pleresident` | legacy-5seg-node | 0.1.1.dev53+g30118ba27 | qwen3.8-flash-next-nvfp4 | rtxpro6000x2-main-native | 미기재 | HINT_MISSING_CERTIFICATE | NVFP4 체크포인트를 RTX PRO 6000 Blackwell ×2(TP=2)에서 **Docker 없이** 524,288 컨텍스트로 |
| `hint/0.18.0/gpt-oss-20b/gb10-main-native/qmxfp4-len131072-kvfp8` | legacy-5seg-node | 0.18.0 | gpt-oss-20b | gb10-main-native | 미기재 | — | GB10 네이티브 예산(외부 이식 타겟 없음) · KV 절대클램프를 실행 하드웨어 자신(121.69GiB×0.90)으로 산출 · 커널 축 부재 확증(attention 후보 ['TRITON_ATTN'] 단일 · MoE marlin) |
| `hint/0.18.0/gpt-oss-20b/gb10-sim-h100/qmxfp4-len131072-kvfp8` | legacy-5seg | 0.18.0 | gpt-oss-20b | gb10-sim-h100 | 미기재 | 미선언 | gpt-oss-20b 를 vLLM 0.18.0 stock wheel 로 GB10 단일노드에 최대 컨텍스트(131,072)로 서빙한 재현 키트. 핵심 발견은 성능이 아니라 **커널 축이 전부 불활성**이라는 사실이다 — 어텐션·MoE 백엔드를 무엇으로 선언해도 엔진은 triton_attn/marlin 을 쓰고, 그 사실이 로그에 남지 않는다. |
| `hint/0.18.0/gpt-oss-20b/gb10-sub-native/qmxfp4-len131072-kvfp8` | legacy-5seg-node | 0.18.0 | gpt-oss-20b | gb10-sub-native | 미기재 | HINT_MISSING_SWEEP_LEVELS | 서브 노드 자율 캠페인 · GB10 네이티브 예산 · KV 61,442 MiB(메인 50,133 대비 +23%) · 완결 엔드포인트 측정(오류 0/18) · 2회 차단(예산 거절 · 워치독 트립) 뒤 수렴 |
| `hint/0.19.0/gpt-oss-120b/gb10x2-cluster-native/qmxfp4-len131072-kvfp8` | legacy-5seg-node | 0.19.0 | gpt-oss-120b | gb10x2-cluster-native | 미기재 | HINT_MISSING_SLAVE_ATTESTATION | GB10 x2 클러스터 · 네이티브 예산(H100 이식 클램프 제거 · KV 2.27배) · overhead 는 KV 의 함수라는 실증 · SoC 열 hard ceiling 이 연속 포화부하 4분에서 실제 벽 |
| `hint/0.19.0/gpt-oss-120b/gb10x2-sim-h100/qmxfp4-len131072-kvfp8` | legacy-5seg | 0.19.0 | gpt-oss-120b | gb10x2-sim-h100 | 미기재 | 미선언 | gpt-oss-120b 를 vLLM 0.19.0 stock wheel 로 GB10 2노드 TP=2 분산 서빙한 재현 키트. 0.19.1 은 이 토폴로지로 뜨지 않으며 회귀 구간은 (0.19.0, 0.19.1] 이다. 커널 축(FlashInfer 어텐션·triton MoE·mxfp4 스위치)은 sm_121a 에서 열리지 않는다. |
| `hint/0.26.0/qwen3-4b/gb10-main-sim-kv24g/len32768-kvauto` | legacy-5seg-node | 0.26.0 | qwen3-4b | gb10-main-sim-kv24g | 미기재 | HINT_MISSING_BENCH_REPORT · HINT_MISSING_CERTIFICATE · HINT_MISSING_LITE | Qwen3-4B · vLLM 0.26.0 소스빌드 · KV 캐시 예산 24GB 모의 · 실험군 A 무양자화(BF16 KV). |
| `hint/0.26.0/qwen3-4b/gb10-main-sim-kv24g/len32768-kvfp8` | legacy-5seg-node | 0.26.0 | qwen3-4b | gb10-main-sim-kv24g | 미기재 | HINT_MISSING_BENCH_REPORT · HINT_MISSING_CERTIFICATE · HINT_MISSING_LITE | Qwen3-4B · vLLM 0.26.0 소스빌드 · KV 캐시 예산 24GB 모의 · 실험군 B FP8. |
| `hint/0.26.0/qwen3-4b/gb10-sub-sim-kv24g/len32768-kvturboquant3bitnc` | legacy-5seg-node | 0.26.0 | qwen3-4b | gb10-sub-sim-kv24g | 미기재 | HINT_MISSING_BENCH_REPORT · HINT_MISSING_CERTIFICATE · HINT_MISSING_LITE | Qwen3-4B · vLLM 0.26.0 소스빌드 · KV 캐시 예산 24GB 모의 · 실험군 D TurboQuant 3bit. |
| `hint/0.26.0/qwen3-4b/gb10-sub-sim-kv24g/len32768-kvturboquant4bitnc` | legacy-5seg-node | 0.26.0 | qwen3-4b | gb10-sub-sim-kv24g | 미기재 | HINT_MISSING_BENCH_REPORT · HINT_MISSING_CERTIFICATE · HINT_MISSING_LITE | Qwen3-4B · vLLM 0.26.0 소스빌드 · KV 캐시 예산 24GB 모의 · 실험군 C TurboQuant 4bit. |
| `hint/0.27.1/qwen3-4b/h10080gb-main-native/len32768-kvauto-pleresident` | legacy-5seg-node | 0.27.1 | qwen3-4b | h10080gb-main-native | 미수령 | 미수령 | — |
| `hint/0.28.0/qwen3.8-27b/rtxpro6000-main-native/len262144-kvfp8` | legacy-5seg-node | 0.28.0 | qwen3.8-27b | rtxpro6000-main-native | 미기재 | HINT_MISSING_LITE | RTX PRO 6000(1of2, 96GB) 네이티브 예산 · bf16가중치+fp8KV — 단일스트림 속도가 fp8가중치 자매셀 대비 약 40%↓(가중치 양자화가 지배) · Docker-in-Docker 불가 호스트라 venv 직접설치+네이티브 프로세스 서빙으로 대체 |
| `hint/0.28.0/qwen3.8-27b/rtxpro6000-main-native/qfp8-len262144-kvauto` | legacy-5seg-node | 0.28.0 | qwen3.8-27b | rtxpro6000-main-native | 미기재 | HINT_MISSING_LITE | RTX PRO 6000(1of2, 96GB) 네이티브 예산 · fp8가중치+KV무양자화 — 동시접속 한계가 fp8KV 자매셀의 절반(batch=3) · Docker-in-Docker 불가 호스트라 venv 직접설치+네이티브 프로세스 서빙으로 대체 |
| `hint/0.28.0/qwen3.8-27b/rtxpro6000-main-native/qfp8-len262144-kvfp8` | legacy-5seg-node | 0.28.0 | qwen3.8-27b | rtxpro6000-main-native | 미기재 | HINT_MISSING_LITE | RTX PRO 6000(1of2, 96GB) 네이티브 예산 · fp8가중치+fp8KV 조합이 최대 동시접속(batch=6, 집계 1181.6 tok/s) · Docker-in-Docker 불가 호스트라 venv 직접설치+네이티브 프로세스 서빙으로 대체 |
| `hint/0.28.0/qwen3.8-27b/rtxpro6000-main-native/qfp8-len524288-kvfp8` | legacy-5seg-node | 0.28.0 | qwen3.8-27b | rtxpro6000-main-native | 미기재 | HINT_MISSING_LITE | RTX PRO 6000(1of2, 96GB) 네이티브 예산 · fp8가중치+fp8KV+YaRN(524288, factor=2.0) — 단일스트림 속도는 non-YaRN 262144 자매셀과 동일(44.27 vs 44.22 t/s) · gen_recipe_set.py 에 hf_overrides SERVE_KNOB 를 신설해야 발행 가능했던 셀 |
| `hint/0.28.0/qwen3.8-27b/rtxpro6000x2-main-native/len262144-kvfp8` | legacy-5seg-node | 0.28.0 | qwen3.8-27b | rtxpro6000x2-main-native | 미기재 | HINT_MISSING_LITE | RTX PRO 6000 x2(전부 사용, TP=2, 192GB) 네이티브 예산 · bf16가중치+fp8KV — 단일스트림 속도가 fp8가중치 자매셀 대비 크게 낮음(정확도 우선 대안) · TP=1 자매셀 대비 속도 배율(~1.75배)이 fp8w 조합(~1.58배)보다 큼 · Docker-in-Docker 불가 호스트라 venv 직접설치+네이티브 프로세스 서빙으로 대체 |
| `hint/0.28.0/qwen3.8-27b/rtxpro6000x2-main-native/qfp8-len1000000-kvfp8` | legacy-5seg-node | 0.28.0 | qwen3.8-27b | rtxpro6000x2-main-native | 미기재 | HINT_MISSING_LITE | RTX PRO 6000 x2(전부 사용, TP=2, 192GB) 네이티브 예산 · fp8가중치+fp8KV+YaRN(factor=4.0, 1,000,000 컨텍스트, vLLM 공식 가이드 상한) — TP=1 캠페인이 "물리적으로 불가"로 추정만 했던 상한에 실측 도달 · 단일스트림 속도는 262144 자매셀과 사실상 동일 · Docker-in-Docker 불가 호스트라 venv 직접설치+네이티브 프로세스 서빙으로 대체 |
| `hint/0.28.0/qwen3.8-27b/rtxpro6000x2-main-native/qfp8-len262144-kvauto` | legacy-5seg-node | 0.28.0 | qwen3.8-27b | rtxpro6000x2-main-native | 미기재 | HINT_MISSING_LITE | RTX PRO 6000 x2(전부 사용, TP=2, 192GB) 네이티브 예산 · fp8가중치+KV무양자화 — TP=1 자매셀 대비 단일스트림 속도 ~1.59배·동시접속 상한 batch=8 · Docker-in-Docker 불가 호스트라 venv 직접설치+네이티브 프로세스 서빙으로 대체 |
| `hint/0.28.0/qwen3.8-27b/rtxpro6000x2-main-native/qfp8-len262144-kvfp8` | legacy-5seg-node | 0.28.0 | qwen3.8-27b | rtxpro6000x2-main-native | 미기재 | HINT_MISSING_LITE | RTX PRO 6000 x2(전부 사용, TP=2, 192GB) 네이티브 예산 · fp8가중치+fp8KV 조합이 이 캠페인의 최대집계처리량(batch=16, 2503.2 tok/s) · TP=1 자매셀 대비 세 배율(속도/batch/집계)이 모두 다르게 스케일 · Docker-in-Docker 불가 호스트라 venv 직접설치+네이티브 프로세스 서빙으로 대체 |
| `hint/0.29.0/qwen3.8-flash-next-fp8/gb10x2-cluster-native/len1048576-kvauto-plemmap` | legacy-5seg-node | 0.29.0 | qwen3.8-flash-next-fp8 | gb10x2-cluster-native | 미기재 | HINT_MISSING_SLAVE_ATTESTATION | Qwen3.8-Flash-Next FP8 · GB10x2 클러스터 · kv=auto · ctx1048576(YaRN f4) · PLE=mmap — decode 35.39 t/s(동시성1) · 예산 floor가 nv4 mmp 셀들보다 타이트(15,280MiB)했으나 mmap이면 통과 |
| `hint/0.29.0/qwen3.8-flash-next-fp8/gb10x2-cluster-native/len1048576-kvfp8e4m3-plemmap` | legacy-5seg-node | 0.29.0 | qwen3.8-flash-next-fp8 | gb10x2-cluster-native | 미기재 | HINT_MISSING_SLAVE_ATTESTATION | Qwen3.8-Flash-Next FP8 · GB10x2 클러스터 · kv=fp8_e4m3 · ctx1048576(YaRN f4) · PLE=mmap — decode 33.27 t/s(동시성1) · mmp 계열 22셀 재수행 캠페인 완결(11/11 성공) |
| `hint/0.29.0/qwen3.8-flash-next-fp8/gb10x2-cluster-native/len262144-kvauto-plemmap` | legacy-5seg-node | 0.29.0 | qwen3.8-flash-next-fp8 | gb10x2-cluster-native | 미기재 | HINT_MISSING_SLAVE_ATTESTATION | Qwen3.8-Flash-Next FP8(Unsloth) · GB10x2 클러스터 · kv=auto · ctx262144 · PLE=mmap — decode 35.37 t/s(동시성1) · 직전 캠페인 "서빙성공·벤치사망" 지점, R3(budget_renew) 교정 후 5레벨 전부 완주 |
| `hint/0.29.0/qwen3.8-flash-next-fp8/gb10x2-cluster-native/len262144-kvfp8e4m3-plemmap` | legacy-5seg-node | 0.29.0 | qwen3.8-flash-next-fp8 | gb10x2-cluster-native | 미기재 | HINT_MISSING_SLAVE_ATTESTATION | Qwen3.8-Flash-Next FP8(Unsloth) · GB10x2 클러스터 · kv=fp8_e4m3 · ctx262144 · PLE=mmap — decode 34.04 t/s(동시성1) · 직전 캠페인 KV캐시산출 직후 워치독 사살(오진) 지점, R1/R2 교정 후 5레벨 전부 완주 |
| `hint/0.29.0/qwen3.8-flash-next-fp8/gb10x2-cluster-native/len524288-kvauto-plemmap` | legacy-5seg-node | 0.29.0 | qwen3.8-flash-next-fp8 | gb10x2-cluster-native | 미기재 | HINT_MISSING_SLAVE_ATTESTATION | Qwen3.8-Flash-Next FP8 · GB10x2 클러스터 · kv=auto · ctx524288(YaRN f2) · PLE=mmap — decode 37.61 t/s(동시성1) · 예산 floor가 res 계열보다도 타이트(15,280MiB)했으나 mmap 이면 통과 |
| `hint/0.29.0/qwen3.8-flash-next-fp8/gb10x2-cluster-native/len524288-kvfp8e4m3-plemmap` | legacy-5seg-node | 0.29.0 | qwen3.8-flash-next-fp8 | gb10x2-cluster-native | 미기재 | HINT_MISSING_SLAVE_ATTESTATION | Qwen3.8-Flash-Next FP8 · GB10x2 클러스터 · kv=fp8_e4m3 · ctx524288(YaRN f2) · PLE=mmap — decode 35.94 t/s(동시성1) · kv dtype 축이 예산 floor 를 바꾸지 않음을 재확인(kv=auto 태그와 완전 동일 floor) |
| `hint/0.29.0/qwen3.8-flash-next-nvfp4/gb10x2-cluster-native/len1048576-kvauto-plemmap` | legacy-5seg-node | 0.29.0 | qwen3.8-flash-next-nvfp4 | gb10x2-cluster-native | 미기재 | HINT_MISSING_SLAVE_ATTESTATION | Qwen3.8-Flash-Next NVFP4 · GB10x2 클러스터 · kv=auto · ctx1048576(YaRN f4) · PLE=mmap — decode 35.92 t/s(동시성1) · R8 YaRN factor=4 첫 실서빙 검증, mmp 계열 캠페인 통산 11/11 성공 |
| `hint/0.29.0/qwen3.8-flash-next-nvfp4/gb10x2-cluster-native/len1048576-kvfp8e4m3-plemmap` | legacy-5seg-node | 0.29.0 | qwen3.8-flash-next-nvfp4 | gb10x2-cluster-native | 미기재 | HINT_MISSING_SLAVE_ATTESTATION | Qwen3.8-Flash-Next NVFP4 · GB10x2 클러스터 · kv=fp8_e4m3 · ctx1048576(YaRN f4) · PLE=mmap — decode 33.40 t/s(동시성1) · R8 YaRN factor=4 2번째 검증(kv dtype 축) |
| `hint/0.29.0/qwen3.8-flash-next-nvfp4/gb10x2-cluster-native/len262144-kvauto-plemmap` | legacy-5seg-node | 0.29.0 | qwen3.8-flash-next-nvfp4 | gb10x2-cluster-native | 미기재 | HINT_MISSING_SLAVE_ATTESTATION | Qwen3.8-Flash-Next-NVFP4 를 GB10 2노드 TP=2 로 세울 때, **PLE 를 NVMe mmap 으로 서빙하는 트랙**은 |
| `hint/0.29.0/qwen3.8-flash-next-nvfp4/gb10x2-cluster-native/len262144-kvauto-pleresident` | legacy-5seg-node | 0.29.0 | qwen3.8-flash-next-nvfp4 | gb10x2-cluster-native | 미기재 | HINT_MISSING_SLAVE_ATTESTATION | Qwen3.8-Flash-Next-NVFP4 를 GB10 2노드 TP=2 로 세울 때, **PLE(per-layer n-gram embedding)를 |
| `hint/0.29.0/qwen3.8-flash-next-nvfp4/gb10x2-cluster-native/len262144-kvfp8e4m3-plemmap` | legacy-5seg-node | 0.29.0 | qwen3.8-flash-next-nvfp4 | gb10x2-cluster-native | 미기재 | HINT_MISSING_SLAVE_ATTESTATION | Qwen3.8-Flash-Next NVFP4 · GB10x2 클러스터 · kv=fp8_e4m3 · ctx262144 · PLE=mmap — decode 38.18 t/s(동시성1) · 직전 캠페인 하네스 오진(3건) 교정 후 첫 라이브 재현 |
| `hint/0.29.0/qwen3.8-flash-next-nvfp4/gb10x2-cluster-native/len524288-kvauto-plemmap` | legacy-5seg-node | 0.29.0 | qwen3.8-flash-next-nvfp4 | gb10x2-cluster-native | 미기재 | HINT_MISSING_SLAVE_ATTESTATION | Qwen3.8-Flash-Next NVFP4 · GB10x2 클러스터 · kv=auto · ctx524288(YaRN f2) · PLE=mmap — decode 33.72 t/s(동시성1) · R8 YaRN 번역기 첫 실서빙 검증 — mrope+partial_rotary_factor 위에서 YaRN 합성 정상 동작 확인 |
| `hint/0.29.0/qwen3.8-flash-next-nvfp4/gb10x2-cluster-native/len524288-kvfp8e4m3-plemmap` | legacy-5seg-node | 0.29.0 | qwen3.8-flash-next-nvfp4 | gb10x2-cluster-native | 미기재 | HINT_MISSING_SLAVE_ATTESTATION | Qwen3.8-Flash-Next NVFP4 · GB10x2 클러스터 · kv=fp8_e4m3 · ctx524288(YaRN f2) · PLE=mmap — decode 34.86 t/s(동시성1) · R8 YaRN 번역기 2번째 실서빙 재현(kv=fp8 조합) |
| `hint/0.29.0rc6/deepseek-v4-flash-0731/gb10x2-cluster-native/len786432-kvfp8-spec7-graph` | legacy-5seg-node | 0.29.0rc6 | deepseek-v4-flash-0731 | gb10x2-cluster-native | 미기재 | HINT_MISSING_BENCH_REPORT · HINT_MISSING_CERTIFICATE · HINT_MISSING_SLAVE_ATTESTATION | stock vLLM 0.29.0rc6 = DS4F-0731 GB10 서빙(포크 불요) · KV는 fp8_ds_mla 단일 경로 · dspark spec7+cudagraph+humming 조합으로 768K 31.12 t/s(+82%) · verdict PASS(explore) |
| `hint/0.29.0rc6/deepseek-v4-flash-0731/gb10x2-cluster-native/spec0-graph0` | legacy-5seg-node | 0.29.0rc6 | deepseek-v4-flash-0731 | gb10x2-cluster-native | 미기재 | HINT_MISSING_BENCH_REPORT · HINT_MISSING_CERTIFICATE · HINT_MISSING_LITE · HINT_MISSING_SLAVE_ATTESTATION | stock vLLM 0.29.0rc6 = DS4F-0731 GB10 서빙(포크 불요) · 셀 b-768k-kvfp8 · 변종 spec0-graph0 · 768K 17.11 t/s @1 · verdict PASS(explore) |
| `hint/0.29.0rc6/deepseek-v4-flash-0731/gb10x2-cluster-native/spec0-graph0-len1m` | legacy-5seg-node | 0.29.0rc6 | deepseek-v4-flash-0731 | gb10x2-cluster-native | 미기재 | HINT_MISSING_BENCH_REPORT · HINT_MISSING_CERTIFICATE · HINT_MISSING_LITE · HINT_MISSING_SLAVE_ATTESTATION | stock vLLM 0.29.0rc6 = DS4F-0731 GB10 서빙(포크 불요) · 셀 e-1m-kvfp8 · 변종 spec0-graph0-len1m · 1M 16.67 t/s @1 · verdict PASS(explore) |
| `hint/0.29.0rc6/deepseek-v4-flash-0731/gb10x2-cluster-native/spec0-graph1` | legacy-5seg-node | 0.29.0rc6 | deepseek-v4-flash-0731 | gb10x2-cluster-native | 미기재 | HINT_MISSING_BENCH_REPORT · HINT_MISSING_CERTIFICATE · HINT_MISSING_LITE · HINT_MISSING_SLAVE_ATTESTATION | stock vLLM 0.29.0rc6 = DS4F-0731 GB10 서빙(포크 불요) · 셀 b-768k-kvfp8-l2graph · 변종 spec0-graph1 · 768K 26.14 t/s @1 · verdict PASS(explore) |
| `hint/0.29.0rc6/deepseek-v4-flash-0731/gb10x2-cluster-native/spec7-graph0` | legacy-5seg-node | 0.29.0rc6 | deepseek-v4-flash-0731 | gb10x2-cluster-native | 미기재 | HINT_MISSING_BENCH_REPORT · HINT_MISSING_CERTIFICATE · HINT_MISSING_LITE · HINT_MISSING_SLAVE_ATTESTATION | stock vLLM 0.29.0rc6 = DS4F-0731 GB10 서빙(포크 불요) · 셀 b-768k-kvfp8-l1spec · 변종 spec7-graph0 · 768K 29.72 t/s @1 · verdict PASS(explore) |
| `hint/0.29.0rc6/deepseek-v4-flash-0731/gb10x2-cluster-native/spec7-graph1-len1m` | legacy-5seg-node | 0.29.0rc6 | deepseek-v4-flash-0731 | gb10x2-cluster-native | 미기재 | HINT_MISSING_BENCH_REPORT · HINT_MISSING_CERTIFICATE · HINT_MISSING_LITE · HINT_MISSING_SLAVE_ATTESTATION | stock vLLM 0.29.0rc6 = DS4F-0731 GB10 서빙(포크 불요) · 셀 e-1m-kvfp8-e1combo · 변종 spec7-graph1-len1m · 1M 30.54 t/s @1 · verdict PASS(explore) |
| `hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvauto-plemmap-spec3-eager` | v6 | 0.29.0rc6 | qwen3.8-flash-next-nvfp4 | gb10-1g2n-cluster-native | full | HINT_MISSING_CERTIFICATE | `nvidia/Qwen3.8-Flash-Next-NVFP4` 를 GB10 1GPU × 2노드(Ray TP=2)에서 vLLM `v0.29.0rc6` 소스빌드로 서빙했다 — NVFP4 · max-model-len 262144 · KV auto · PLE NVMe mmap · MTP k=3 · eager, 판정점 동시성 1 decode 20.98 t/s(MTP on · GuideLLM full 반복 3). 가장 비싼 벽은 stock 이 NVFP4 mixed 체크포인트를 싣지 못한 것(W1 → 빌드 패치 60)과 통합메모리에서 PLE 테이블을 거둘 경로가 없던 것(W6 → 62 mmap + 노드 로컬 NVMe 스테이징)이다. 성능은 탐색 합격선 대비 REFUTE 이고(기능은 PASS · 인증서 없음) 이전 인증 측정과의 큰 격차는 원인 미분석(Q1) — 이 수치를 기준선으로 쓰지 말 것 · KV 20GiB 클램프(V6)는 필요량을 크게 넘긴 음성대조값이다. |
| `hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvfp8-plemmap-spec3-eager` | v6 | 0.29.0rc6 | qwen3.8-flash-next-nvfp4 | gb10-1g2n-cluster-native | lite(선언) | BENCH_MODE_LITE · HINT_MISSING_CERTIFICATE · HINT_MISSING_SWEEP_LEVELS | `nvidia/Qwen3.8-Flash-Next-NVFP4` 를 GB10 1GPU × 2노드(Ray TP=2)에서 vLLM `v0.29.0rc6` 소스빌드로 서빙했다 — NVFP4 · max-model-len 262144 · KV fp8_e4m3 · PLE NVMe mmap · MTP k=3 · eager, lite 관측 warm decode 18.77 t/s(동시성 1 · MTP on · 반복 1 · OBSERVATION-ONLY · 판정 없음). 가장 비싼 벽은 stock 이 NVFP4 mixed 체크포인트를 싣지 못한 것(W1 → 빌드 패치 60), fp8 KV 를 QSA 가 거부한 것(W8 → 64), 통합메모리에서 PLE 테이블을 거둘 경로가 없던 것(W6 → 62 mmap + 노드 로컬 NVMe)이다. 오해 위험: KV 20GiB 클램프(V6)는 필요량을 크게 넘는 승계 손레버이고, 리포트의 cold TTFT 는 연속 두 번째 실행의 값이라 cold 가 아니다(Q2) — 이 lite 수치를 기준선이나 full · 인증 수치와 나란히 쓰지 말 것. |
<!-- hint-index:rows -->
