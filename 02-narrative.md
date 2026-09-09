# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26090912_ds4f0731_광의탐색_셀루프_서사.md`
- testlog: `../testlog/testlog_26090912_ds4f0731_029rc6_광의탐색_종합판정.md`

## 서사

1. **빌드**: 0.29.0rc6은 torch 2.13.0 핀 → source-build(NGC 26.07). 1차 빌드는 constraint(0.28.0 baseline)의 `flashinfer-python==0.6.16.post3`와 상류 선언 `==0.6.18`이 ResolutionImpossible로 충돌 — constraint를 상류 선언에 양보해 해소(양노드 동일 실패로 requirements 클래스 확정). (근거: testlog_26090904 §1)
2. **서빙 진입의 4연속 fail-closed**: master/slave 컨테이너명 미emit → KV 절대클램프 미선언(155GiB 모델은 단일노드 선측정 불가라 멀티 직접 수렴으로 전환 · 시드=10GiB) → env 인라인 주석 오염 → `distributed-executor-backend: ray` 누락. 전부 가드가 정상 차단한 것이며 우회 0. (근거: testlog_26090904 §2)
3. **arch-wall 중재**: 옛 벽(flashinfer decode_dsv4 page_block64)은 0.29.0rc6 stock이 해소 — 대신 `fp8_ds_mla layout only supports fp8 kv-cache` assert가 KV 축을 fp8 단일로 강제(sm_121a→SM120 클래스 강제 선택 · turboquant/auto 구조적 거부). 이 사실로 캠페인 KV 축이 fp8 단일로 개정됐다. (근거: testlog_26090904 §3 · trial-1c 엔진 로그)
4. **레버 측정**: eager 기준선 17.11 t/s → cudagraph 26.14 → dspark spec(nspec7) 29.72 → **결합 31.12 t/s(+81.9%)** — 이 태그의 구성. spec accept_len 2.53(엔진 실측). (근거: sweep_map_26090912_b768k_levers.md · 종합판정 §스코어보드)
5. **운영**: 기동 밸리(155GiB 로드의 페이지캐시)가 memwatch 절대 플로어(10240MiB)를 치는 구간이 있어 KV 클램프는 그 밸리까지 포함해 10GiB로 수렴했다. 벤치 전 캐시 드롭 필수. (근거: devlog_26090912 §수렴의 3단계)

## 되풀이하지 말 것

- **kv auto/turboquant 탐색** — 엔진이 assert로 거부(fp8_ds_mla 포맷). 이 모델+칩에서 시간을 쓰지 마라. (근거: testlog_26090904 §3)
- **moe-backend=triton** — 엔진 거부("Mxfp4 MoE backend TRITON does not support SILU"). DS4F(fp8·silu)는 humming이 정답. (근거: 종합판정 §스코어보드 L3 · serve_fail_b-768k-kvfp8-l3moe 로그)
- **0.25.1의 MATMUL_DECODE 레버 기대** — 0.29에 그 env가 없다. 레버는 버전마다 재확인. (근거: 종합판정 §구조 판정)
- **18GiB/14GiB KV 클램프** — KV 용량으로는 성립하나 호스트 기동 밸리+벤치 상주가 절대 플로어와 충돌해 memwatch 킬 2회. 10GiB가 이 호스트의 binding. (근거: devlog_26090912 §수렴의 3단계)
