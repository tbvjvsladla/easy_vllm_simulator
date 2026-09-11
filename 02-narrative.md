# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26091121_qwen38fn_22셀재수행_캠페인완주_서사.md`
- testlog: `../testlog/testlog_26091121_qwen38fn_22셀재수행_캠페인완주_판정.md`

## 서사

이 모델(Qwen3.8-Flash-Next-NVFP4)을 1,048,576토큰(1M) 컨텍스트로 서빙하려면 네이티브 최대
위치 인코딩(262,144)을 4배 확장해야 한다. YaRN(`rope_type: yarn`)으로 `factor: 4`,
`original_max_position_embeddings: 262144`를 config.yaml의 `hf-overrides`에 인라인 주입해
해결했다 — 512k 구간에서는 `factor: 2`로 동일 메커니즘이 이미 검증됐고(devlog §"배경"), 이번이
factor=4의 첫 실서빙 검증이다(testlog §"R8 실서빙 재검증 — factor=4 완결").

PLE(Persistent Layer Expert) mmap 모드가 이 캠페인 전체(262k~1m, 11/11 셀)에서 서빙 성공의
결정적 축임이 재확인됐다(testlog §"패턴 최종 확정"). PLE resident 모드는 같은 조건에서 항상
실패한다 — NVFP4 변종은 예산 선판정을 통과하고도 로드 중 실측 상주메모리가 예측을 초과해
호스트 워치독에 사살되고(devlog §"시도 — res 계열"), FP8 변종은 애초에 예산 선판정 단계에서
음수 floor로 즉시 차단된다. 이 체크포인트(nv4-bf-1m-mmp)는 mmap 모드를 썼기 때문에 성공했다.

## 되풀이하지 말 것

- **YaRN factor를 컨텍스트 배율과 헷갈리지 말 것**: factor는 "네이티브 대비 배율"이지 절대
  토큰 수가 아니다. 262k 네이티브 기준으로 512k=factor 2, 1m=factor 4다.
  `original_max_position_embeddings`는 항상 262144로 고정 — 이 값을 실수로 바꾸면 위치
  인코딩이 깨진다.
- **PLE resident 모드로 1m 컨텍스트를 시도하지 말 것**: 이 체크포인트 크기(NVFP4 123.57GiB)
  에서는 예산 선판정이 양수 floor(16,064MiB)를 내더라도 실측 로드 중 반드시 워치독에 죽는다
  (캠페인 전체 5/5 재현, devlog §"시도 — res 계열"). PLE mmap 모드를 기본으로 쓴다.
