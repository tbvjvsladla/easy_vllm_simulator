## 읽을 원재료
- devlog: ../devlog/devlog_26090912_ds4f0731_광의탐색_셀루프_서사.md
- testlog: ../testlog/testlog_26090912_ds4f0731_029rc6_광의탐색_종합판정.md

## 서사 (셀 한정 추가)
이 셀은 위 벽 지도의 ②(fp8 KV)·④(moe humming)·⑤(spec+graph 결합) 축의 부분 조합이다:
- 레버 0/1: spec off·cudagraph off → 기준선
- 레버 1: spec only → dspark nspec7 on (accept_len ~2.36)
- 레버 2: graph only → cudagraph FULL_AND_PIECEWISE on
- 레버 4: spec+graph → 승자 (L4가 본 레시피)

(각 변종별 핵심 차이는 곧 트리플렛 3개 파일에 담겼다.)

## 되풀이하지 말 것
- kv auto/turboquant 탐색 · moe=triton · 0.25.1 MATMUL_DECODE 기대 · 18/14GiB KV 클램프
- 풀 근거는 종합판정 §스코어보드 및 §구조판정 참조
