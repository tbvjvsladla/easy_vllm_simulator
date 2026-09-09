# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

이 태그는 **explore 권한**(문턱 없는 지도)으로 측정됐다 — verdict=PASS 는 게이트가 아니라 서술이다. 측정 자체는 full 모드(GuideLLM 0.7.3 · in 1024/out 256 · 레벨 1~16)로 전 레벨 완주했다.

- 이 구성(셀 b-768k-kvfp8-l1spec · 변종 spec7-graph0 · 768K): decode **29.72 t/s** @동시성1 (cudagraph on · synthetic in1024/out256 · GuideLLM) · verdict PASS(authority=explore — 문턱 아닌 지도).
     기준선 대비: eager 17.11 → +73.7%. like-with-like: R_token(spec-aware) — spec off 수치와 직접 비교 금지. 동급 메트릭은 같은 스윕의 다른 셀로 비교 가능. spec accept_len ≈ 2.36(엔진 실측).

말할 수 없는 것: 외부 레퍼런스 대비 우위(절대 문턱) · 인증서급 재현성 주장 — 이 둘은 인증서 없이는 성립하지 않는다.
