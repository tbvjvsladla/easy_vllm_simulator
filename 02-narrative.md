# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26091321_qwen38fn_네이티브_9셀_full_bench_라운드.md`
- testlog: `../testlog/testlog_26091321_qwen38fn_네이티브_9셀_full_bench_라운드.md`

## 서사

이 형상(NVFP4 · PLE resident · 1048576 컨텍스트)에 닿기까지 넘은 벽은 넷이고, 넷 다 **정적 검사로는
안 잡히는 종류**다 — 인자는 엔진까지 도달하는데 조용히 안 먹거나, 여유가 있을 때는 드러나지 않는다.

1. **정식 릴리스가 로드 자체를 거절한다.** `ValueError: There is no module or parameter named
   'ngram_embedding.weight_scale' in Qwen4ExpNGramEmbedding`. 0.29.0 의 PLE quant method 선택이
   `Fp8Config` 전용이라 NVFP4 가 unquantized 경로로 가고, 그 경로엔 체크포인트의 scale 을 받을
   파라미터가 없다. 해소는 **nightly 커밋 핀**이다 — 성능 선택이 아니라 구동 전제다.
   (근거: testlog_26091314 §대조셀 판정 — vLLM 0.29.0 정식은 이 NVFP4 체크포인트를 서빙할 수 없다)
2. **YaRN 확장이 조용히 무효가 된다.** `User-specified max_model_len ... is greater than the derived
   max_model_len (max_position_embeddings=...)`. yarn 계열은 factor 를 곱하지 않고
   `max_position_embeddings` 를 그대로 상한으로 쓴다. override 에 **그 값을 함께** 실어야 한다.
   ★ 게이트는 "rope 인자가 있다"만 보므로 통과시킨다 — **존재와 충분함은 다른 술어다.**
   (근거: testlog_26091314 §YaRN 무효 — 512K·1M 에서 0.14% 오차로 검증)
3. **KV 절대클램프가 12% 모자라 즉사한다.** full-attention 층만으로 산정하면 부족하다 —
   `linear_attention` 36층의 conv/ssm 상태가 같은 KV 풀에서 잡힌다. 실측 보정계수 **≈1.12**.
   클램프에 여유가 있으면 안 드러나고 얇은 구성에서만 터진다.
   (근거: testlog_26091314 §KV 하이브리드 보정 — 512K 6.74 vs 6.73 GiB, 1M 13.48 vs 13.46 GiB)
4. **같은 형상을 두 번째로 띄우면 기동이 33분으로 늘어지고 죽는다.** 이전 런이 남긴 FlashInfer
   autotune 캐시를 읽는 순간 두 랭크가 갈린다 — 한쪽은 `Config cache hit` 로 즉시 빠지고 다른
   쪽은 전량 재튜닝에 들어가는데, 튜닝 타이밍이 world cpu group 으로 **평균되는 집합연산**이라
   합류하지 않는 랭크가 생기면 남은 랭크의 매 라운드가 ~40s 로 늘어지고 결국
   `gloo Connection closed by peer` → `Engine core initialization failed` 가 난다.
   해소는 **세션마다 빈 autotune 캐시 디렉터리를 가리키는 것**이다(삭제 ✗ · 격리 ○).
   (근거: testlog_26091321 §4 결함 ① — 33분/크래시 → 8.3초로 검증)

## 되풀이하지 말 것

- **KV dtype 을 fp8 로 낮추려 하지 마라.** 이 아치 전용 백엔드가 막고 있다 —
  `Qwen4ExpQSAFlashAttentionBackend.supported_kv_cache_dtypes = ['auto','bfloat16']`. 같은 파일의
  일반 FlashAttention 은 fp8 을 받는데 이 백엔드만 닫혀 있다. **fp8 KV 를 전제한 용량 계획은 이
  아치에서 성립하지 않는다** — 우리가 이 축을 먼저 시도했고 닫혀 있음을 확인했다.
- **포크를 짜기 전에 `main` 을 먼저 읽어라.** 선행 계획서가 자체이식하려던 패치 2종이 착수 시점엔
  upstream 정식 기능이 돼 있었다. 포크 준비가 가장 비싼 단계인데 **가장 싼 확인을 건너뛰면** 그
  비용을 다 치른다.
- **teardown 뒤 VRAM 을 반드시 확인하라.** 엔진 자식 프로세스(`VLLM::Worker_TP*`)는 argv 를 스스로
  갈아치워서 프로세스 이름 패턴에 안 걸린다. 부모만 죽이면 자식이 VRAM 을 쥔 채 남고, 다음 실행이
  "메모리 없음"으로 죽는다 — **사인이 원인에서 두 단계 떨어진다.** 프로세스 그룹으로 내려라.
  (근거: testlog_26091321 §4 결함 ② — 83.8 GiB/GPU 잔존 실측)
- **lite 수치와 full 수치를 섞지 마라.** 같은 셀에서 최대 11% 차이가 난다(워밍 조건이 다르다).
