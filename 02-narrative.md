# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26091121_qwen38fn_22셀재수행_캠페인완주_서사.md`
- testlog: `../testlog/testlog_26091121_qwen38fn_22셀재수행_캠페인완주_판정.md`

## 서사

nv4-bf-1m-mmp(kv=auto)와 동일한 YaRN factor=4 오버라이드 위에서, kv-cache-dtype만 fp8_e4m3로
바꾼 두 번째 검증이다(testlog §"R8 실서빙 재검증"). KV dtype 축은 서빙 성립에 영향을 주지 않고
decode t/s만 소폭(35.92→33.40) 달라졌다 — kv=auto가 이 조합에서 살짝 더 빠른 경향은 262k·512k
그룹에서도 일관됐다(devlog 참조).

## 되풀이하지 말 것

- kv-cache-dtype을 바꿔도 YaRN 오버라이드 재계산은 불필요하다 — 두 축(context 확장, KV 양자화)
  은 독립이다.
- PLE resident 모드로 이 체크포인트(NVFP4)를 1m에서 시도하지 말 것 — `nv4-f8-1m-res`가 같은
  기전(워치독 사살, kv dtype 무관)으로 실패했다(testlog §"비-measured 셀").
