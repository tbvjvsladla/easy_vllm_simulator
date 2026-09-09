# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26090914_run_trial_native모드_추가_및_광의탐색_셀A.B.C_구성.md`
- testlog: `../testlog/testlog_26090915_Qwen3.8.27B_광의탐색_3셀_FullBench_비교.md`

## 서사

이 태그는 자매 hint `hint/0.28.0/qwen3.8-27b/rtxpro6000-main-native/qfp8-len262144-kvfp8`
(셀 B)와 **동일한 환경·동일한 벽**을 겪었다(같은 서빙 세션, 같은 광의탐색 캠페인의 다른 셀) —
Docker 부재(devlog §"무엇을 했나")·torchvision 누락·TP 오염(testlog §알려진 결함/한계)·
tool_call_parser 오독 방지는 그쪽 태그 본문을 참조하라(중복 서술 대신 인용). 이 셀 고유의
차이점만 아래에 적는다.

**증상(이 셀 고유)**: KV 캐시를 양자화하지 않고(fp16/bf16 기본) fp8 가중치만 쓰면, 같은
262144 컨텍스트·같은 GPU 예산에서 fp8 KV 셀(B)보다 훨씬 낮은 batch(동시접속) 한계에 부딪힌다.
**원인**: KV 캐시 바이트가 무양자화(2바이트/토큰)라 fp8(1바이트/토큰) 대비 2배 크다 — 같은
VRAM 예산 안에서 담을 수 있는 (batch × context) 곱이 절반이다. **해소**: batch=32 요청 시
필요 KV(572.3GB) > 실측 가용(54.44GiB) → 선형 스케일로 batch=3까지 하향(3×16.66GB≈50GB, 예산
내). KV 절대 클램프(kv-cache-memory-bytes)는 이 batch 로 재수렴시켜야 한다(recipe.py simulate
trial-loop 2회: 측정→클램프산출→검증).

## 되풀이하지 말 것

1. Docker/torchvision/TP/tool_parser 벽은 자매 hint(B) 본문 §1·§3 참조 — **동일하게 적용된다**.
2. **"KV 무양자화 = 정확도 우선"이라는 인상만으로 batch 를 크게 잡지 마라.** KV 무양자화는
   토큰당 바이트가 2배라 같은 GPU 예산에서 fp8 KV 대비 동시접속 여력이 훨씬 작다(이 캠페인
   실측: batch 상한 3 vs 6). 정확도-처리량 트레이드오프를 미리 계산하고 batch 를 정하라 —
   요청한 batch 가 예산을 넘으면 트라이얼루프가 `vram_infeasible` 로 HITL 에스컬레이션한다
   (자동 하향 조정 없음, lock 변수라서).
