# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26091314_qwen38fn_네이티브_14셀_캠페인_완주.md`
- testlog: `../testlog/testlog_26091314_qwen38fn_네이티브_14셀_캠페인_완주.md`

## 서사

이 레시피는 "빠른 설정"이 아니라 **네 개의 벽을 지난 경로**다.

**① 정식 릴리스로는 애초에 뜨지 않는다.** vLLM 0.29.0 정식은 NVFP4 체크포인트를 **가중치 로딩에서**
거절한다 — `ValueError: There is no module or parameter named 'ngram_embedding.weight_scale' in
Qwen4ExpNGramEmbedding`. 0.29.0 의 PLE quant method 선택이 `Fp8Config` 전용이라 NVFP4 는
unquantized 경로로 가는데 그 경로엔 체크포인트의 `weight_scale` 을 받을 자리가 없다.
따라서 **nightly 커밋 핀은 성능 선택이 아니라 구동 가능성의 전제**다
(근거: `testlog_26091314` §셀 사인 `c0-stable-nv4-dev-262k`).

**② KV 양자화 축은 이 아치에서 열리지 않는다.** `Qwen4ExpQSAFlashAttentionBackend.
supported_kv_cache_dtypes = ['auto','bfloat16']` — 같은 파일의 일반 FlashAttention 은 fp8 을
받는데 이 아치 전용 백엔드만 막혀 있다(설치본 런타임 실측). 선행 계획서가 세웠던
`kv{bf16,fp8}` 축은 **성립하지 않는다**. 24셀 매트릭스가 12셀로 줄어든 이유가 이것이다.

**③ KV 소요를 full-attention 층만으로 계산하면 12% 모자란다.** 이 모델은 하이브리드라
`linear_attention` 36층의 conv/ssm 상태가 **같은 KV 풀에서** 잡힌다. 262,144 기준 내 공식 3,072 MiB
대 엔진 실제 요구 3,451 MiB(계수 1.1233). 클램프에 여유가 있으면 드러나지 않고, 여유가 얇은
구성에서만 터진다. 엔진이 정확한 숫자를 주므로 **거절 메시지를 읽으면 된다**
(근거: `testlog_26091314` §캠페인 중 발견·교정된 결함).

**④ MTP 가 이 모델의 지배적 레버다.** `--speculative-config
'{"method":"qwen4_exp_mtp","num_speculative_tokens":3}'` 로 **2.13~2.33배**가 나온다
(엔진 실측 acceptance length 2.63~3.47). 이걸 켜지 않고 남의 수치와 비교하면 조건이 달라진다 —
공개 기준선 다수가 MTP 포함 수치다.


## 되풀이하지 말 것

**버린 경로와 그 이유** — 다음 사람이 다시 걷지 않도록 적는다.

- **자체 이식 3종(계획했다가 폐기)** — 착수 전에 upstream 을 먼저 읽어라. 계획서가 이식하려던
  NVFP4 PLE 와 PLE 오프로드는 **이미 정식 기능**이었다. 포크·패치를 짜기 전에 `main` 의 해당
  모듈을 확인하는 것이 가장 싼 단계다.
- **`kv_cache_dtype: fp8`(불가)** — 위 ②. 패치로 열 수 있는 종류가 아니다(백엔드 클래스의 선언).
- **PLE 오프로드를 기본으로 삼는 것(조건부 권장으로 하향)** — 262K 에서는 offload 와 device 가
  같지만, NVFP4 는 확장 컨텍스트에서 offload 가 ~8% 비용을 문다. 반대로 FP8 은 같은 축에서
  offload 가 **오히려 +4.2%** 였다. **방향이 변종마다 반대이므로 일반 법칙으로 쓰지 마라** —
  기전은 규명하지 못했다(음성정직). 실용 규칙: **VRAM 이 허락하면 device, 부족하면 offload.**
- **`fp8` + PLE device + 512K 이상(구조적 불가)** — weights 88,464 MiB/GPU 라 KV 가 들어갈 자리가
  없다. 512K 는 6.73 GiB 필요 · 5.66 가용, 1M 은 13.46 필요 · 5.53 가용. FP8 로 긴 컨텍스트를
  쓰려면 **offload 가 선택이 아니라 필수**다.
- **`fp8` + device + 262K 에 MTP 추가(불가)** — 그 구성이 이미 VRAM 98.8% 를 쓴다. MTP 모듈이
  요청하는 160 MiB 가 없어 OOM 이다. 최빠듯 구성에서는 레버가 닫힌다.
- **512K 를 중간 타협점으로 고르는 것(비권장)** — 컨텍스트 확장 비용은 262K→512K 에서 거의 다
  치러지고 512K→1M 은 추가 비용이 사실상 없다. **512K 를 쓸 바엔 1M 을 쓰는 편이 낫다.**

