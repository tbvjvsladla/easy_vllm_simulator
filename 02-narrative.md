# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26090602_캠페인재개_2브랜치_서사.md`
- testlog: `../testlog/testlog_26090602_캠페인재개_2브랜치_광의탐색_판정.md`

## 서사

### 증상 — 커널을 골랐다고 믿었는데, 고른 적이 없었다

이 조합(vLLM 0.18.0 · gpt-oss-20b · aarch64 sm_121a)에서 **어텐션·MoE 백엔드를 선언해도 엔진이
그것을 쓰지 않는다.** 그런데 서빙은 성공하고 벤치도 정상이며 **로그에 거부가 없다.**

- `attention_backend=FLASHINFER` → 실측 `triton_attn` (testlog §1 표 A3)
- `attention_backend=FLASH_ATTN` → 실측 `triton_attn` (동 A4)
- `moe_backend=triton` / `flashinfer_trtllm` / `cutlass` → 전부 실측 `marlin` (동 A5·A6·A9)

### 원인 — 후보목록이 하나뿐이고, 나머지는 조용히 되돌아간다

0.18.0 이 이 디바이스에서 여는 어텐션 후보는 `['TRITON_ATTN']` 하나다. MoE 는 선언이 무엇이든
mxfp4 경로에서 `marlin` 이 낙찰된다. **거부가 아니라 폴백**이라 로그에 흔적이 남지 않는다
(testlog §1.2).

같은 벽이 vLLM **0.19.0 에서는 소리를 낸다** — 같은 `moe=triton` 선언이
`ValueError: Mxfp4 MoE backend 'TRITON' does not support the deployment configuration since kernel
does not support current device cuda.` 로 죽는다(testlog §2.1). ⚠ 다만 그 대조는 버전·모델·
토폴로지가 함께 바뀐 관측이라 **버전 단독 귀속은 아직 못 한다**(동 §2.1 경고 박스).

### 해소 — 고칠 것이 없다. **알아내는 것**이 해소였다

성능은 어떤 축을 움직여도 **44.53 ~ 45.59 t/s (2.4% 폭)** 안에 있고, 그 안에서 **가장 빠른 것이
아무것도 선언하지 않은 기준선**이다(testlog §1.1). 이 레시피의 정답은 "튜닝하지 않는 것"이다.

대신 **계측이 대사해야 한다.** 셀 레코드에 실측-대-선언 대사 필드
(`attention_backend_mismatch`·`moe_backend_mismatch`)를 넣자 즉시 5건이 걸렸다(devlog §3.1).
그 필드가 없었다면 "FlashInfer MoE 로 44.74 t/s" 라는 **거짓 기록**이 남았을 것이다.

### 재현 확인 — 세 번의 독립 측정이 0.7% 안에서 만났다

같은 레시피를 세 경로로 쟀다: 메인 노드의 캠페인 기준선 셀 **45.59**, 두 번째 GB10 노드가 **자기
이미지를 스스로 빌드해** 자율 수행한 벤치 **45.28**, 그리고 이 키트를 정규 이름으로 재서빙한
재측정 **45.54** t/s. 셋의 폭이 **0.7%** 다.

**이미지는 전송하지 않았다.** 같은 태그인데 이미지 ID 가 노드마다 다르다는 것이 그 증거다
(testlog §3.1). 크기 차이(33.4 GB vs 22.0 GB)는 스토리지 드라이버 차이일 뿐이므로 **완료 판정에
크기를 쓰면 위양성**이다 — 판정은 ID 와 기능 프로브(`--gpus=all` 필수)로 한다.

## 되풀이하지 말 것

1. **KV `bfloat16` 을 값으로 적지 마라.** 엔진은 `kv_cache_dtype == "auto" or startswith("fp8")`
   만 받는다. 비양자화 KV 를 원하면 값은 **`auto`** 다(testlog §1.3). 이걸 몰라 셀 하나를 날렸다.
2. **`fp8_e5m2` 는 0.18.0 에서 열리지 않는다.** `{fp8, fp8_e4m3}` 로 제한된다(동 §1.3).
3. **mxfp4 전용 FlashInfer 스위치 두 개는 이 디바이스에서 무효과다.**
   `VLLM_USE_FLASHINFER_MOE_MXFP4_BF16` / `..._MXFP8` 을 켜도 서빙은 되지만 백엔드는 marlin 이고
   수치도 기준선과 구분되지 않는다(44.70 / 44.61). 0.19.0 에서는 아예 거부한다.
4. **동시성을 공식으로 계산하지 마라.** 공식(`layers×kv_heads×head_dim×2×dtype`)은 18.67 을 주는데
   엔진 실측은 **36.72x** 였다 — 정확히 2배다. gpt-oss 가 교대 층에 `sliding_window=128` 을 쓰고
   vLLM 하이브리드 할당기가 그 층의 KV 를 거의 잡지 않기 때문이다. **serve 로그의
   `Maximum concurrency` 두 줄을 읽어라**(측정 > 공식).
5. **harmony 계열을 `--backend openai-chat` 으로 재지 마라.** `--ignore-eos` 가 무력해지고 서버
   harmony 파서가 스트림 중 깨져 `HarmonyError` 로 측정이 무효가 된다(실측 error_rate 0.25).
   `--backend openai` 를 쓴다.
6. **도는 스크립트를 편집하지 마라.** bash 는 파일을 **바이트 오프셋으로** 읽으므로, 실행 중인
   스크립트에 줄을 더하면 문법적으로 멀쩡한 파일이 `syntax error` 를 낸다(devlog §5).
