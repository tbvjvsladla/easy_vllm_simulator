# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26090308_gpt_oss_20b_gb10_0180.md`
- testlog: `../testlog/testlog_26090308_gpt_oss_20b_gb10_0180.md`

## 서사

**증상 1 — 요청한 attention backend가 조용히 무시됨.** Phase 2 초회 lockset은
`attention_backend=FLASHINFER`를 요청했다. 그러나 엔진 로그에 `Unknown vLLM environment variable
detected: VLLM_ATTENTION_BACKEND` 경고가 찍혔고, 실제 선택은 `Using TRITON_ATTN attention backend
out of potential backends: ['TRITON_ATTN']`였다(testlog "Phase 2 trial-loop" 절 인용). **원인**:
vLLM 0.18.0에서 gpt-oss+mxfp4 조합은 TRITON_ATTN이 유일한 가용 백엔드이고, 요청 자체가 무효한 환경변수라
아무 효과가 없었다(devlog §2). **해소**: attention_backend를 실측대로 TRITON_ATTN으로 정정 — 이 조합에서는
선택의 여지가 없으므로 요청이 아니라 확인의 문제였다.

**증상 2 — batch=32에서 호스트 워치독에 의한 외부 SIGKILL.** 언클램프(trial1, `kv_cache_memory_bytes=null`)
측정에서 kv_cache_gib=87.72GiB(실측 24.58KB/token), weights=13.72GiB로 프로파일까지는 성공했으나,
gmu가 호스트 안전계층에 의해 0.90→0.852로 자동 하향됐음에도 컨테이너가 138.6초 만에 CUDA OOM 예외 없이
죽었다(testlog "Phase 2 trial-loop" 표, trial 1 행). `classify_failure`는 `vram_infeasible`로 판정했다
(`required_kv=105,142,151,354B > max_safe_kv=100,470,151,321B`). **원인**: GB10은 통합메모리 호스트라
vLLM 자체의 gmu 안전장치를 통과해도, 그와 **별개인 호스트 레벨 워치독**이 CUDA 예외 없이 개입할 수
있다 — "측정된 최대치에 근접"이 곧 "안전"은 아니었다. **해소**: max-model-len(131072, 전략상 고정)은
그대로 두고 batch를 32→16으로 낮추고 `kv-cache-memory-bytes=51539607552`(48GiB)를 명시적 절대클램프로
지정했다. trial2는 예산 선언이 정상 발행되고 CUDA 그래프 캡처까지 마친 뒤 `classify → none`(수렴)으로
끝났다(testlog 동일 표, trial 2 행).

**증상 3 — harmony 포맷이 벤치 측정을 왜곡할 뻔함.** gpt-oss는 harmony의 assistant-action stop 토큰이
EOS와 별개로 턴을 끝내므로, `/v1/chat/completions`(기본 backend)로 측정하면 `--ignore-eos`가 무력화돼
TPOT이 실제보다 느리게 측정된다(이 프로젝트에서 과거 3.3배 왜곡이 실측된 바 있음 — testlog "Full
벤치마크" 절). **해소**: 처음부터 `run_bench.sh --backend openai`(`/v1/completions`)로 측정했다 —
총 생성 토큰이 요청한 4096(=16×256)과 정확히 일치했고 `client_engine_agreement.ratio=1.0`으로 왜곡
없음을 확인했다(testlog 동일 절).

## 되풀이하지 말 것

1. **`VLLM_ATTENTION_BACKEND=FLASHINFER`를 gpt-oss+mxfp4에 지정하지 말 것** — vLLM 0.18.0에서는
   미인식 환경변수로 조용히 무시된다. 요청이 반영됐는지는 항상 엔진 로그의 "Unknown vLLM environment
   variable" 경고와 "potential backends" 목록으로 확인하라.
2. **언클램프 trial에서 측정된 kv_cache_gib 값을 그대로 다음 절대클램프로 쓰지 말 것** — GB10 같은
   통합메모리 호스트에서는 호스트 워치독이 vLLM의 CUDA OOM 예외 없이 개입한다. 측정치보다 확실히
   낮은(이 캠페인에서는 87.72GiB→48GiB, 약 45% 절감) 값으로 시작하라.
3. **gpt-oss 계열을 `/v1/chat/completions`(기본 backend)로 벤치하지 말 것** — `--backend openai`로
   `/v1/completions`을 명시해야 harmony ignore-eos 무력화에 의한 TPOT 왜곡을 피한다.
4. **`docker compose build`로 wheel 트랙 이미지를 새로 만들려 하지 말 것** — 이 브랜치의 compose
   `build:` 스탠자는 canonical default_track(source-build)을 가리키므로 무관한 트랙이 빌드된다.
   `docker build -f Dockerfile -t <tag> .`을 직접 써라.
