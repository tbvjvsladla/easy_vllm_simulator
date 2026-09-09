# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26090917_TP2_GPU2장_전부사용_광의탐색_캠페인_4셀.md`
- testlog: `../testlog/testlog_26090917_Qwen3.8.27B_TP2_광의탐색_4셀_FullBench_비교.md`

## 서사

이 태그는 이전 TP=1 캠페인(자매 hint `hint/0.28.0/qwen3.8-27b/rtxpro6000-main-native/
qfp8-len262144-kvauto`, 셀 A)과 같은 축 조합(fp8 가중치, KV 무양자화, 262144)이지만 **GPU
2장 전부(TP=2)를 쓴다**는 점이 유일한 차이다. Docker 부재·torchvision 누락·tool_call_parser
오독 방지 등은 그 태그와 동일 — 여기서 반복하지 않는다.

**증상(이 셀 고유)**: `--kv-cache-dtype none` 으로 첫 서빙 시도가 즉사했다(vLLM 이 받는 값이
아님 — 유효값은 `auto`/`fp8`/... 이지 `none` 이 아니다). **원인**: candidate 스키마에서
`kv_cache_quant` 를 문자열 `"none"` 으로 선언했는데, 트리플렛 생성기의 `_kv_dtype_flag` 는
falsy 값(`null`/빈문자열)만 "미지정(=auto)"으로 처리하고 truthy 문자열은 그대로 CLI 에
넘긴다 — `"none"` 은 truthy라 그대로 `--kv-cache-dtype none` 이 되어 거부당했다(원인:
`docs/devlog/devlog_26090917_TP2_GPU2장_전부사용_광의탐색_캠페인_4셀.md` §5 참조).
**해소**: `kv_cache_quant` 를 `null` 로 선언(자매 셀 A 와 동일 관례).

**신규 코드 gap(이 캠페인이 처음 발견)**: `run_trial.py` 의 트라이얼 경로가 `tensor_parallel_size`
를 전혀 읽지 않아 TP=2 를 요청해도 트라이얼이 TP=1 로 로드되고 있었다(배포 3종 세트는 이미
TP>1 을 정확히 반영하고 있었으므로, 검증한 것과 배포되는 것이 갈리는 상태였다). 이 hint 를
발행한 프로젝트에서는 이미 교정됐다(`--tensor-parallel-size` 를 candidate 값으로 추가) —
**다른 프로젝트/포크에서 TP>1 을 처음 시도한다면 같은 gap 이 있는지 먼저 확인하라**(구체적으로:
트라이얼 실행기가 실제로 `--tensor-parallel-size` 플래그를 CLI 에 싣는지 로그로 확인).

## 되풀이하지 말 것

1. Docker/torchvision/tool_parser 벽은 자매 hint(셀 A) 본문 §1·§3 참조 — **동일하게 적용된다**.
2. **KV 무양자화를 candidate 에 문자열 `"none"` 으로 적지 마라** — `null`(미지정)로 적어야
   `--kv-cache-dtype` 플래그 자체가 생략되고 vLLM 기본(auto) 동작을 얻는다. 문자열 `"none"` 은
   그대로 CLI 에 실려 즉사한다.
3. **트라이얼 경로가 실제로 TP>1 을 요청하는지 확인 없이 "3종 세트에 TP 가 적혀 있으니 됐다"고
   가정하지 마라** — 배포 산출물과 검증 산출물은 별개의 코드 경로를 탄다. 이 hint 를 만든
   프로젝트에서는 그 gap 이 실제로 있었고(트라이얼이 조용히 TP=1 로 돌고 있었다), 발견은
   우연이었다(TP=2 를 처음 시도했기 때문).
