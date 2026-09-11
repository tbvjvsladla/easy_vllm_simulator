# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26091114_qwen38fn_22셀재수행_7셀체크포인트_서사.md`
- testlog: `../testlog/testlog_26091114_qwen38fn_22셀재수행_7셀체크포인트_판정.md`

## 서사

**증상(직전 캠페인)**: `camp-26090918`에서 이 축(context 262144→524288, YaRN factor=2)을 포함한
512k/1m 셀 8개가 ValidationError 로 즉사했고, void_reason 은 "YaRN 확장 미적용(셀 축에 미포함)"
으로 **원인이 반대로** 기록됐다 — 실제로는 YaRN 을 서빙 인자로 번역하는 실행자가 저장소에
아예 없었다(`docs/testlog/testlog_26091109_..._판정.md` §2.7). **원인**: vLLM 은
`max_model_len > native max_position_embeddings` 를 로드 진입에서 즉시 거부하는데, 트리플렛에
`hf-overrides` rope 확장 인자가 없었다. **해소**: `check_smoke_model.py` 가 로드 0초에 그 부재를
지목하고 정확한 병합 rope 파라미터(`mrope_interleaved`+`mrope_section`+`partial_rotary_factor`
+`rope_theta` 전부 보존)를 출력하도록 R8 교정 — 그 줄을 트리플렛에 추가해 재시도하니 정상
로드·완결 엔드포인트 추론 정합("The capital of France is" → " Paris, ..., population of
2,148,27...") · 5레벨 완주(33.72 t/s). `mrope_interleaved=True`+`partial_rotary_factor=0.25`
위에서 YaRN 합성이 이 하드웨어·이 vLLM 에서 **최초로 실측 검증**됐다.

## 되풀이하지 말 것

**컨텍스트를 네이티브 `max_position_embeddings` 이상으로 늘릴 때는 항상 `hf-overrides` rope
확장 인자가 필요한지부터 확인하라** — 없이 로드를 시도하면 ValidationError 즉사이고, 그 죽음의
원인을 "YaRN이 안 먹었다"로 오독하기 쉽다(직전 캠페인이 정확히 이 실수를 했다). 이 모델은
`mrope_interleaved`+`mrope_section=[11,11,10]`+`partial_rotary_factor=0.25` 를 함께 쓰므로,
rope 딕셔너리를 **통째로 교체하지 말고 기존 키를 전부 보존한 채 `rope_type`/`factor`/
`original_max_position_embeddings` 만 추가**하라 — 교체하면 mrope·부분회전이 조용히 사라진다.
