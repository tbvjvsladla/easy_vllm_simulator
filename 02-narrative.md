# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26091117_qwen38fn_22셀재수행_512k그룹완료체크포인트_서사.md`
- testlog: `../testlog/testlog_26091117_qwen38fn_22셀재수행_512k그룹완료체크포인트_판정.md`

## 서사

**증상**: FP8 체크포인트(172.78GiB) + kv_cache_dtype=fp8_e4m3 + context=524288(YaRN f2) 조합.
같은 체크포인트의 kv=auto 태그(`len524288-kvauto-plemmap`)가 이미 예산 floor 15,280MiB로
같은 그룹의 resident 셀보다도 타이트한 마진에서 성공한 바 있었다 — 이 셀은 kv dtype만 바꿔
그 결과가 재현되는지 확인하는 자리였다.

**원인/우려**: kv_cache_dtype을 fp8로 바꾸면 KV 풀의 메모리 사용 패턴이 달라질 수 있어(같은
체크포인트의 262k 그룹에서 kv=fp8 축이 실제로 KV 풀 확장 잠재를 가진다는 주석이 트리플렛에
있었다), floor 산식 자체는 kv dtype 무관(weights_mib만 기준)이지만 실측 상주가 예측과
달라질 위험을 배제할 수 없었다. 262144 네이티브를 넘는 컨텍스트라 R8(YaRN) 오버라이드도
필요했다.

**해소**: 예산 선판정 floor=15,280MiB(kv=auto 태그와 완전 동일값 — R2 산술이 kv dtype과
무관함을 다시 확인) → 정상 로드(약 24분) → 헬스 200 · 기능 스모크 통과 → 5레벨 벤치 완주,
decode t/s(동시성=1)=35.94, verdict=PASS(explore). FP8+mmap 조합은 kv dtype 축(auto/fp8)
양쪽 모두에서 성공해, mmp 계열 512k 그룹 4/4 완결의 마지막 조각이 됐다.

## 되풀이하지 말 것

- **kv_cache_dtype이 예산 선판정 floor를 바꾸지 않는다고 가정해도 된다** — 이 캠페인 전체
  (262k·512k, NVFP4·FP8, mmap·resident 전 조합)에서 kv dtype이 floor 값을 바꾼 사례가
  0건이었다. `kv-cache-memory-bytes`(절대 클램프, 20GiB)가 context 길이·kv dtype과 무관한
  고정값이기 때문이다. 다만 이것이 KV 풀 자체의 실제 확장 여부까지 보증하지는 않는다 —
  이 하네스는 클램프를 절대값으로 두므로 fp8이 이론상 압축을 제공해도 예산 계산에는 반영되지
  않는다는 점을 혼동하지 마라.
- **같은 체크포인트의 resident 조합은 kv dtype과 무관하게 실패한다**(`fp8-f8-512k-res`,
  floor=−9,134MiB — kv=auto인 `fp8-bf-512k-res`와 완전 동일값) — 이 태그와는 다른 실패 기전.
