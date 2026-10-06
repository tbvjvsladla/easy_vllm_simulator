# DeepSeek-V4.1-Flash 2노드 EXL3 캠페인 결산

> DeepSeek-V4.1-Flash 아종(EXL3 A·B·C · 원본 · NVFP4) · 2×GB10(TP=2 · Ray) · vLLM 0.30.0 자체 이식 · 1M 컨텍스트
> 캠페인 `camp-26100320-v41flash-2node-broad`(선행 `camp-26100214-v41flash-multi-1m`) · 2026-10-02 ~ 10-06 · **종결 — 후속 캠페인 없음**
> 출처: 캠페인 testlog·bench_report·hint 태그 실측. **지도이지 SLA 보증이 아니다.**

## 요약

| 항목 | 값 |
|---|---|
| 목표 | 여러 V4.1-Flash 아종을 2노드에서 1M 으로 띄우고 최적 서빙전략을 찾는다 · 저자 런타임을 쓰지 않고 **빌드/런타임 패치로 내재화**한다 |
| 승자 | **A(sfxnz EXL3 2.0bpw) · 우리 0.30.0 이식 정식 이미지 · 셀 `i5-a-kern`** |
| 승자 성능 | lite **44.74** t/s · L.A.I.L **39.94** t/s · Full(GuideLLM 1024/256 · c=1 × 3) **34.41(cold) · 40.83 · 39.80** t/s |
| 출발점 | 같은 팩 eager · 스펙 끔 13.90 t/s(lite) → 그래프+DSpark 정식 18.49(L.A.I.L) |
| 참조 천장 | 저자 런타임(R-1) lite 39.74 · L.A.I.L 44.50 — **lite 는 넘었고 L.A.I.L 은 약 90%** |
| 1M 기준 | 1M 기동 + lite PASS · 125k 장문은 열 한계(SoC ≥92 °C 롤링 가드)로 미완 |
| 배포 | hint 태그 `hint/0.30.0/deepseek-v4.1-flash-exl3-2.0bpw-sfxnz/gb10-1g2n-cluster-native/qexl3-len1048576-kvfp8-spec3-densegemv-moeprep` |

## 아종별 결론

| 아종 | 결과 | 사유 |
|---|---|---|
| **A** sfxnz EXL3 2.0bpw | **서빙 · 측정 · hint 발행** | 우리 이식(Engram 파일 백엔드 + 로컬 NVMe 스테이징)으로 노드 상주가 안전 바닥(MemAvailable ≥10 GiB) 안에 든다 |
| B Mia EXL3 2.9bpw | void | 가중치 상주가 안전 바닥과 양립 불가(로드 중 워치독 트립) |
| C coolbho3k EXL3 3bpw | 재현 불가로 기록(서빙 미시도) | 원형 레시피가 호스트 변경(디스플레이 드라이버·headless 재부팅) · 안전 바닥 완화(0.5~0.8 GiB) · compose 셀별 override · GHCR 이미지와 드래프터 획득을 모두 요구 |
| 원본 FP8 | void(게이트 거부) | 상주 286.42 GiB ÷ TP2 + 바닥 > 노드 가용 |
| NVFP4 | void(게이트 거부) | 상주 302.25 GiB ÷ TP2 + 바닥(=164,991 MiB/노드) > 가용 119,203 MiB — 계획 산술과 일치 |

## 내재화 사다리 — 무엇이 속도를 만들었나

수락 길이는 처음부터 저자와 같았다(≈2.1). 격차는 **스텝 비용**이었고, 커널 단위 프로파일(torch profiler · rank0)이 레버를 골랐다.

| 단계 | 패치(슬롯) | L.A.I.L t/s | 스텝 GPU 시간 |
|---|---|---|---|
| 그래프 NaN 해소 | PRE 89 — 캡처 전 더미 forward 가 오염시킨 KV null 블록을 캡처 후 0 으로 | 17.91 | ≈119 ms |
| K1 | POST 73 — 저자 p2b MoE 체인(coop dataflow) | 23.88 | ≈89 ms |
| K2 | PRE 90 — o_proj wo_a fp8 einsum + NCCL tail | 25.35 | ≈85 ms |
| (K3) | Engram census — 노출 ≈3 ms 로 **병목 아님** 확인(비동기화 기각) | — | — |
| **K4** | PRE 91 — dense MXFP8 GEMM 을 cutlass → **b12x**(프로파일상 스텝의 61% = 54 ms) | **39.39** | 54 → 20 ms(dense) |
| K5 | PRE 92 — 저자 dense GEMV(9 모양 · 비트 자가시험) | 39.84 | ≈51 ms |
| K6 | POST 74 — MoE 준비 glue 융합(moe_prep_fused) | 42.02 | −2.4 ms |
| **정식(i5)** | 위 전부를 정식 `Dockerfile.source-build` 로 재빌드 · 양 노드 정합 4축 equal | 39.94(수락 2.05) | — |

- 저자 패치 5종은 고정 커밋(MIT)의 원문을 빌드 시점에 받아 트리 digest·파일 sha256 을 검증하고 **그대로** 적용했다. 우리 편집은 serve-time A/B 손잡이와 실패 시 원인을 드러내는 로그뿐이다.
- 가장 큰 단일 레버는 **dense MXFP8 GEMM 의 백엔드 선택**(cutlass → b12x)이었다 — 추정이 아니라 커널 프로파일로 찾았다.
- L.A.I.L 은 회차별 수락 길이(2.05~2.26)에 ±5% 흔들린다. lite 는 같은 레시피에서 41.5~44.7.

## 벤치 · 열

- Full bench(lite ∪ GuideLLM × 3)는 이번 캠페인부터 수행했다. 기본 워크로드(1024/256)는 장문 열 한계와 무관하다.
- **열이 Full 의 실질 상한이다**: c=2 를 포함한 연속 Full 은 SoC 93~96 °C 에서 세션 열 가드(롤링 60 s 중 50 s ≥92 °C)를 두 번 발동시켰다. 가드가 집행한 스윕은 Full 근거에서 제외했고, 승자의 Full 근거는 **c=1 만 반복한 기동(열 kill 없음)** 이다.
- weak 루브릭 판정(PASS)은 정보가 없다 — roofline 이 EXL3·Engram 의 활성 바이트를 과대 계산해 바닥이 1.5 t/s 로 나온다.

## 이번 캠페인이 고친 하네스

- 외부 저장소 `git clone` 자율성 확장(메인·서브 · 빌드 입력은 커밋 고정 · 가중치 저장소는 획득 게이트).
- Engram 파일 백엔드 선언을 RAM 게이트·스모크가 상주에서 제외.
- flashinfer-cubin 휠(서명 URL 1시간 만료)을 청크별 재해석 이어받기 + sha256 검증으로 받는 빌드 패치.
- hint 발행기: 보존된 lite 측정과의 결정론 조인 · 엔진 로그 머리 보존 · lite 수락 길이 · 현행 Full 정의 판독 · lite 리포트 quantization·TP 행.
- 공통층은 single-node 브랜치에도 전파했다.

## 정직성 노트

- 한 스윕을 처음에 "Full 판정 유지"로 보고했으나, 세션 열 가드 kill 이 낀 스윕이라 기준에 따라 제외하고 c=1 만 다시 쟀다.
- hint 는 lite 등급 측정에 묶였다(이미 묶인 발행 기록은 등급을 올릴 수 없다). Full 수치는 셀 판정 문서에서 다른 기동의 측정으로 인용된다.
- 세션 단절로 서버가 측정 없이 약 6 시간 떠 있었다 — 이후 1시간 주기 진척 점검으로 보완했다.

## 종결

- 사용자 결정(2026-10-06): **이 캠페인으로 종결하고 후속 캠페인을 열지 않는다.**
- 미착수로 남긴 것(재개 계획 없음 · 기록만): B 재시도 · C 팩의 우리 이식 · NCCL tail 단독 A/B · 125k 장문 열 대책.
- 미해결 하네스 관찰(기록만): 세션 열 가드 kill 이 블랙박스 원장에 남지 않음 · roofline 의 EXL3/Engram 과대 · 서브 배달 평면이 빌드 패치를 덮지 않음 · 개정으로 더한 셀은 배정 밖이라 hint 를 낼 수 없음.
