# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26091121_qwen38fn_22셀재수행_캠페인완주_서사.md`
- testlog: `../testlog/testlog_26091121_qwen38fn_22셀재수행_캠페인완주_판정.md`

## 서사

FP8 체크포인트(172.78GiB)는 NVFP4(123.57GiB)보다 커서 같은 mmap 조건에서도 예산 선판정
floor가 더 좁다(이 셀 15,280MiB — nv4 mmp 셀들은 40,478MiB). 그럼에도 mmap 모드에서는 좁은
floor가 실패를 뜻하지 않는다 — 512k 그룹의 동형 셀(`fp8-bf-512k-mmp`)도 같은 floor 값으로
정상 완주했고, 1m에서도 재현됐다(testlog §"판정 결과").

## 되풀이하지 말 것

- floor의 절댓값으로 성패를 예단하지 말 것 — mmap/resident 구분이 유일한 신뢰할 만한 예측
  변수다. `fp8-bf-1m-res`(같은 체크포인트, resident 모드)는 floor가 **음수**(−9,134MiB)로
  로드 0초에 즉시 차단됐다 — mmap과 resident는 weights_mib 계산식 자체가 달라 floor 격차가
  20,000MiB 이상 벌어진다(devlog §"시도 — res 계열").
